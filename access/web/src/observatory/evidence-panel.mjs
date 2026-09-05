import {createToSQueryOperations} from '../query-operations';
import {KnowledgeClient,RequestSlots,RequestError,localized} from './knowledge-client.mjs';
import {loadEvidence,compareEvidence,sourceRefs,selectionSummary} from './evidence-model.mjs';
import {refreshIcons} from './icons';

const el=(tag,text='',className='')=>{const node=document.createElement(tag);node.textContent=text;node.className=className;return node;};
const button=(label,action,className='')=>{const node=el('button',label,className);node.type='button';node.addEventListener('click',action);return node;};
const labels={
  'pre-canon':'До канона','canon':'Канон','derived-export':'Проекция источников',
  prepared_research_candidate:'Исследовательский кандидат',prepared_branch_candidate:'Кандидат ветви',
  contested_review_required:'Требует рассмотрения',pending_human_review:'Ожидает рассмотрения',
  'not-recorded':'Не указан',unresolved:'Не разрешено',review_status_unresolved:'Статус рассмотрения не установлен',
  contested_by:'Оспаривается',uncertain_relation:'Неопределённая связь',polemicizes_with:'Полемизирует с',
};
const human=value=>labels[value]||value||'Не указан';

export function createEvidencePanel(root,scene,panels,{selected,onUserAction}){
  const client=new KnowledgeClient(),requests=new RequestSlots();
  const queries=createToSQueryOperations(async(url,options={})=>{
    const signal=AbortSignal.any([options.signal||new AbortController().signal,AbortSignal.timeout(60000)]);
    try{
      const response=await fetch(url,{...options,signal});
      if(!response.ok)throw new RequestError(response.status,'Не удалось получить основания. Попробуйте ещё раз.');
      return await response.json();
    }catch(error){
      if(signal.aborted)throw signal.reason;
      if(error instanceof TypeError)throw new Error('Нет связи с данными. Повторите запрос после подключения.');
      throw error;
    }
  });
  const panel=el('section','','sc-panel sc-evidence');panel.hidden=true;panel.setAttribute('aria-label','Основания и прочтения');
  panel.innerHTML='<div class="sc-panel-top"><span class="sc-eyebrow">ЛИСТ ИССЛЕДОВАНИЯ</span><button type="button" class="sc-icon sc-evidence-close" aria-label="Закрыть основания"><i data-lucide="x" aria-hidden="true"></i></button></div><div class="sc-evidence-heading"><span class="sc-evidence-symbol" aria-hidden="true">✧</span><div><p class="sc-evidence-kind"></p><h3></h3></div></div><div class="sc-evidence-tabs" role="tablist" aria-label="Основания и сравнение"></div><div class="sc-evidence-body" id="sc-evidence-content" role="tabpanel" tabindex="0"></div><div class="sc-evidence-status" role="status"></div><div class="sc-evidence-footer"><span>От мысли — к источнику</span></div>';
  root.append(panel);
  const body=panel.querySelector('.sc-evidence-body'),status=panel.querySelector('.sc-evidence-status'),tabs=panel.querySelector('.sc-evidence-tabs');
  let target=null,result=null,failure=null,active='grounds',focusReturn=null,viewKey='',requestId=0;
  panels.register('evidence',panel,()=>{requests.cancelAll();body.setAttribute('aria-busy','false');});
  function close(){panels.close('evidence');onUserAction();
    const star=[...root.querySelectorAll('.sc-node')].find(node=>node.dataset.id===target?.raw.id&&!node.hidden);
    (focusReturn?.isConnected&&!focusReturn.closest('[hidden]')?focusReturn:star||root.querySelector('.sc-overview')).focus();
  }
  panel.querySelector('.sc-evidence-close').addEventListener('click',close);
  panel.addEventListener('keydown',event=>{if(event.key==='Escape'){event.preventDefault();event.stopPropagation();close();}});
  const tabIds=['grounds','compare'];
  const tabButtons=['Основания','Сравнение'].map((label,index)=>{
    const tab=button(label,()=>{onUserAction();switchTab(tabIds[index]);});tab.id='sc-evidence-'+tabIds[index];tab.setAttribute('role','tab');tab.setAttribute('aria-controls',body.id);tabs.append(tab);return tab;
  });
  tabs.addEventListener('keydown',event=>{if(!['ArrowLeft','ArrowRight','Home','End'].includes(event.key))return;event.preventDefault();
    const next=event.key==='Home'?0:event.key==='End'?1:1-tabButtons.indexOf(event.target);onUserAction();switchTab(tabIds[next]);tabButtons[next].focus();
  });
  function switchTab(tab){active=tab;panel.dataset.tab=tab;
    tabButtons.forEach((button,index)=>{button.setAttribute('aria-selected',String(tabIds[index]===tab));button.tabIndex=tabIds[index]===tab?0:-1;});
    body.setAttribute('aria-labelledby','sc-evidence-'+tab);render();
  }
  function section(title,content,className=''){
    const node=el('section','','sc-evidence-section '+className);node.append(el('h4',title));for(const item of content)node.append(typeof item==='string'?el('p',item):item);return node;
  }
  function refs(references,title='Источники'){
    const details=el('details','','sc-evidence-refs');details.append(el('summary',title+' · '+references.length));
    for(const ref of references){const row=el('div','','sc-evidence-ref');
      if(/^https?:\/\//i.test(ref)){try{const url=new URL(ref),a=el('a',url.hostname+url.pathname);a.href=url.href;a.target='_blank';a.rel='noopener noreferrer';row.append(a);}catch{row.append(el('span',ref));}}
      else row.append(el('span',ref));
      row.append(button('Копировать',async()=>{try{await navigator.clipboard.writeText(ref);status.textContent='Ссылка на источник скопирована.';}catch{status.textContent='Выделите и скопируйте путь к источнику.';}},'sc-evidence-copy'));details.append(row);
    }
    return details;
  }
  function posture(raw){
    const row=el('dl','','sc-evidence-posture');for(const [label,value]of [['Слой',raw.epistemic?.authority_layer],['Рассмотрение',raw.epistemic?.review_posture],['Канон',raw.epistemic?.canon_status]]){const pair=el('div');pair.append(el('dt',label),el('dd',human(value)));row.append(pair);}return row;
  }
  function points(title,values,className=''){
    if(!values?.length)return null;const list=el('ul');for(const value of values)list.append(el('li',value));return section(title,[list],className);
  }
  function append(...nodes){body.append(...nodes.filter(Boolean));}
  function provenance(){
    const raw=result?.raw||target.raw;
    append(posture(raw),refs(sourceRefs(raw),'Происхождение объекта'));
    const route=button('Открыть досье источников',()=>{onUserAction();root.dispatchEvent(new CustomEvent('sophia-sources',{detail:{raw,kind:target.kind}}));},'sc-evidence-source');body.append(route);
  }
  function renderGrounds(){
    const raw=result.raw,packet=result.packet;
    if(!packet){
      append(section('Происхождение', [localized(target.kind==='relation'?raw.display.explanation:raw.display.summary,'Описание пока не записано.')],'sc-evidence-finding'));
      append(section('Маршрут оснований', [result.availability==='outside_route'?'Объект не найден в доступной области Evidence Lens. Это не означает, что у него нет оснований.':'Для этого слоя отдельный маршрут оснований ещё не подключён. Здесь показаны сведения из карточки и её источники.']));provenance();return;
    }
    append(section('Что установлено', [packet.finding_ru||packet.finding],'sc-evidence-finding'));
    const limits=el('div','','sc-evidence-conclusions');
    for(const node of [points('Можно утверждать',packet.conclusion.allowed_ru||packet.conclusion.allowed,'sc-evidence-allowed'),points('Вывод пока не следует',packet.conclusion.not_allowed_ru||packet.conclusion.not_allowed,'sc-evidence-limits')])if(node)limits.append(node);
    append(limits);
    if(packet.conclusion.can_conclude!==true)append(el('p','Материала недостаточно для окончательного вывода в указанной области.','sc-evidence-note'));
    append(points('Открытые вопросы',packet.gaps_ru||packet.gaps));
    if(packet.source_anchors.length){const anchors=el('details','','sc-evidence-refs');anchors.append(el('summary','Точные фрагменты · '+packet.source_anchors.length));for(const anchor of packet.source_anchors){const item=el('div','','sc-evidence-ref');item.append(el('p',(anchor.anchor_segment_ids||[]).join(' · ')),el('small',anchor.witness_scope||''));if(anchor.relation_ref)item.append(refs([anchor.relation_ref],'Запись связи'));anchors.append(item);}append(anchors);}
    if(packet.routes.length){const routes=el('details','','sc-evidence-refs');routes.append(el('summary','Маршруты к основаниям · '+packet.routes.length));for(const route of packet.routes){const item=el('div','','sc-evidence-ref');item.append(el('small',human(route.route_kind)+' · '+human(route.status)));if(route.ref)item.append(refs([route.ref],'Открыть путь'));routes.append(item);}append(routes);}
    append(refs(packet.source_refs,'Источники поля оснований'));provenance();
  }
  function readingCard(reading,caption,isSelected=false){
    const card=el('article','','sc-reading'+(isSelected?' sc-reading-selected':''));
    card.append(el('span',caption,'sc-reading-caption'),el('h4',human(reading.label)));
    if(reading.route)card.append(el('p',reading.route,'sc-reading-route'));
    card.append(el('p',reading.statement||'Описание этого прочтения пока не записано.','sc-reading-text'));
    if(reading.review_posture||reading.canon_status)card.append(el('p',[reading.review_posture,reading.canon_status].filter(Boolean).map(human).join(' · '),'sc-evidence-note'));
    card.append(refs(reading.source_refs));return card;
  }
  function renderComparison(){
    if(!result.packet){append(section('Сопоставление ещё не подключено',[result.availability==='outside_route'?'Объект находится за пределами доступного маршрута оснований.':'В этом слое пока нет подключённого поля прочтений. Соседство на карте само по себе не означает разногласия.']));provenance();return;}
    const comparison=compareEvidence(result,selectionSummary(result.raw,target.kind)),packet=result.packet,raw=result.raw;
    append(el('p','Сопоставление показывает записанные связи и вопросы к ним. Их истинность определяется рассмотрением источников.','sc-evidence-note'));
    const readings=comparison.competing_readings;
    const chosen={label:localized(raw.display.title||raw.display.label),statement:localized(target.kind==='relation'?raw.display.explanation:raw.display.summary),route:localized(raw.display.statement),source_refs:sourceRefs(raw),review_posture:raw.epistemic?.review_posture,canon_status:raw.epistemic?.canon_status};
    const spread=el('div','','sc-reading-spread');spread.append(readingCard(chosen,'ВЫБРАНО',true));
    if(readings.length){
      const right=el('div','','sc-reading-alternative');
      const label=el('label','Сопоставить с');label.htmlFor='sc-reading-choice';const select=el('select');select.id=label.htmlFor;
      for(const [index,reading]of readings.entries()){const option=el('option',`${index+1}. ${human(reading.label)}${reading.route?' · '+reading.route:''}`);option.value=String(index);select.append(option);}
      const card=el('div');const update=()=>{card.replaceChildren(readingCard(readings[Number(select.value)],'ДРУГОЕ ПРОЧТЕНИЕ'));scene.invalidate();};
      select.addEventListener('change',()=>{onUserAction();update();});right.append(label,select,card);update();spread.append(right);
    }else spread.append(section('Других прочтений не показано',['В полученной области нет других оспаривающих связей. Это не означает согласия или доказанности.'],'sc-reading-empty'));
    append(spread);
    const coverage=packet.coverage;
    append(el('p',`Получено оспаривающих связей: ${coverage.returned_challenge_relations??packet.challenge_relations.length} из ${coverage.available_challenge_relations??packet.challenge_relations.length}.`+(target.kind==='relation'?' Выбранная связь исключена из второго столбца.':''),'sc-evidence-note'));
    if(comparison.contextual_readings.length){const context=el('details','','sc-evidence-refs');context.append(el('summary','Контекст · '+comparison.contextual_readings.length));for(const reading of comparison.contextual_readings)context.append(readingCard(reading,'СВЯЗЬ КОНТЕКСТА'));append(context);}
    append(points('Что остаётся открытым',comparison.gaps));
  }
  function render(){
    body.replaceChildren();status.textContent='';body.setAttribute('aria-busy',String(!result&&!failure));
    if(failure){append(section('Не удалось прочитать основания',[failure.message]),button('Повторить',()=>{onUserAction();void open(target,active).catch(()=>{});},'sc-evidence-source'));}
    else if(!result){append(el('div','Собираю источники и прочтения…','sc-evidence-loading'));}
    else if(active==='grounds')renderGrounds();else renderComparison();
    body.scrollTop=0;scene.invalidate();
  }
  async function open(source,tab='grounds',{signal,limit=60}={}){
    if(!source?.raw)throw new Error('Сначала выберите звезду или отношение.');
    const ticket=++requestId;
    focusReturn=document.activeElement;panels.open('evidence');target=source;result=null;failure=null;
    const revision=scene.port.packet?.source_revision;viewKey=JSON.stringify([source.raw.id,source.kind,revision]);
    panel.dataset.itemId=source.raw.id;panel.querySelector('h3').textContent=human(localized(source.raw.display.title||source.raw.display.label));
    panel.querySelector('.sc-evidence-kind').textContent=source.kind==='relation'?'ОТНОШЕНИЕ':localized(source.raw.display.kind_label,'УЗЕЛ').toUpperCase();
    switchTab(tab);tabButtons[tabIds.indexOf(tab)].focus();
    try{
      const response=await requests.run('evidence',ownSignal=>loadEvidence(source.raw,source.kind,revision,{client,queries,limit,signal:signal?AbortSignal.any([signal,ownSignal]):ownSignal}));
      signal?.throwIfAborted();if(!response.current||panel.hidden)throw new DOMException('Panel closed','AbortError');
      result=response.value;render();return result;
    }catch(error){
      if(ticket===requestId&&!panel.hidden){failure=error.name==='AbortError'?new Error('Чтение прервано. Можно повторить запрос.'):error;render();}
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
    if(!loaded.packet)throw new Error('Для этого объекта маршрут Evidence Lens сейчас недоступен.');
    return tab==='compare'?compareEvidence(loaded,selection):{...loaded.packet,binding:loaded.binding,agent_summary:{...loaded.packet.agent_summary,selection:raw.id}};
  }
  refreshIcons();window.addEventListener('pagehide',()=>requests.cancelAll());
  return {selectionChanged,handlers:{'tos.page.inspect-epistemic':(input,execution)=>command('grounds',input,execution),'tos.page.compare-readings':(input,execution)=>command('compare',input,execution)}};
}
