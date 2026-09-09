import {ui,uiText} from './ui-i18n.mjs';
import {formView,inspectExactHumanForms} from './human-forms.mjs';
import {renderContextData} from './context-view.mjs';

const el=(tag,value='',className='')=>{const node=document.createElement(tag);node.className=className;uiText(node,value);return node;};
const roleLabels={name:'Название',caption:'Подпись',hover:'Краткий контекст',statement:'Формулировка',grounds:'Основания',history:'История',technical:'Точные сведения'};
const stateLabels={ready:'Доступна',missing:'Форма не предоставлена',unavailable:'Форма недоступна',ambiguous:'Есть несколько форм',
  'over-budget':'Форма превышает предел доставки',invalid:'Пакет формы некорректен',stale:'Форма устарела',restricted:'Доступ ограничен','needs-assessment':'Требуется оценка'};
const reasonLabels={'exact-language':'Точное совпадение языка','less-specific-language':'Показан менее конкретный вариант языка',
  automatic:'Автоматический выбор источника',fallback:'Выбранный язык отсутствует; показан доступный',original:'Исходная форма, как указано источником',
  'no-ready-form':'Готовая форма не предоставлена','multiple-forms':'Однозначная форма не выбрана','original-role-not-declared':'Исходная форма не указана',
  'inspect-exact-form':'Полный пакет не поместился в ответ'};
function jsonBlock(value,anchor){
  const node=el('pre',JSON.stringify(value,null,2),'sc-form-json');node.dir='auto';node.dataset.readingAnchor=anchor;return node;
}

function scalarValue(value){
  if(typeof value==='string')return value;
  if(value===null)return 'null';
  if(typeof value==='boolean')return value?'true':'false';
  if(typeof value==='number')return String(value);
  return '';
}

// Context values are source data, not a browser taxonomy. Render their keys
// and scalar values as a readable projection, while the exact JSON stays
// available below the corresponding details disclosure.
function readableValue(value,anchor,depth=0){
  if(value===null||['string','boolean','number'].includes(typeof value)){
    const node=el('span',scalarValue(value),'sc-form-value');node.dir='auto';node.dataset.readingAnchor=anchor;return node;
  }
  if(depth>=8){
    const node=el('span',ui('Вложенное значение доступно в точной записи.'),'sc-form-value sc-form-value-raw');node.dir='auto';node.dataset.readingAnchor=anchor;return node;
  }
  if(Array.isArray(value)){
    const list=el('ul','','sc-form-values');list.dataset.readingAnchor=anchor;
    if(!value.length)list.append(el('li','[]','sc-form-value'));
    value.forEach((item,index)=>{const row=el('li');row.append(readableValue(item,`${anchor}:${index}`,depth+1));list.append(row);});
    return list;
  }
  if(value&&typeof value==='object'){
    const list=el('dl','','sc-form-values');list.dataset.readingAnchor=anchor;
    const entries=Object.entries(value);
    if(!entries.length)list.append(el('dd','{}','sc-form-value'));
    entries.forEach(([key,item])=>{
      const row=el('div');row.append(el('dt',key,'sc-form-value-key'),readableValue(item,`${anchor}:${key}`,depth+1));list.append(row);
    });
    return list;
  }
  return el('span','sc-form-value');
}

function exactDetails(title,value,anchor,className='sc-form-exact'){
  const details=el('details','',className);details.append(el('summary',title));details.append(jsonBlock(value,anchor));return details;
}

function appendContextEntry(section,entry,anchorPrefix){
  const context=el('section','','sc-form-context');context.dataset.contextSlot=entry.slot;
  const heading=el('p',entry.slot,'sc-form-context-slot');heading.dataset.readingAnchor=`${anchorPrefix}:slot`;context.append(heading);
  context.append(renderContextData(entry.value,`${anchorPrefix}:value`));
  if(typeof entry.binding?.pointer==='string'){
    const pointer=el('p',ui('Поле источника: {0}',[entry.binding.pointer||ui('корень')]),'sc-source-ref');
    pointer.dataset.readingAnchor=`${anchorPrefix}:pointer`;context.append(pointer);
  }
  context.append(exactDetails(ui('Точная запись контекста'),entry,`${anchorPrefix}:raw`));
  section.append(context);
}

function appendReadyPacket(section,packet,role,anchorPrefix=`form:${role}`){
  section.append(el('p',ui('Язык формы: {0}',[packet.language||ui('Не указан')]),'sc-reader-language-note'));
  if(packet.derivation==='source-copy')section.append(el('p',ui('Полная копия поля источника; истинность формулировки не оценивалась.'),'sc-form-status'));
  const wording=el('div',packet.display_text,'sc-form-wording');wording.dir='auto';if(packet.language)wording.lang=packet.language;
  wording.dataset.readingAnchor=anchorPrefix+':wording';section.append(wording);
  if(packet.context.length){
    section.append(el('h6',ui('Обязательный контекст')));
    packet.context.forEach((entry,index)=>{
      appendContextEntry(section,entry,`${anchorPrefix}:context:${index}`);
    });
  }
  if(packet.language_context){
    section.append(el('h6',ui('Языковое происхождение')));
    const languageContext=packet.language_context.value||{};
    const summary=el('p',ui('Язык: {0} · связь: {1}',[languageContext.language||ui('не указан'),languageContext.relation||ui('не указана')]),'sc-reader-language-note');
    summary.dataset.readingAnchor=anchorPrefix+':language:summary';section.append(summary);
    section.append(exactDetails(ui('Точная запись языкового контекста'),packet.language_context,anchorPrefix+':language:raw'));
  }
  const metadata=el('details','','sc-form-metadata');metadata.append(el('summary',ui('Происхождение и точная версия формы')));
  const {display_text,context,language_context,...rest}=packet;metadata.append(jsonBlock(rest,anchorPrefix+':metadata'));section.append(metadata);
  if(packet.assessment_snapshot) section.append(el('p',ui('Оценка относится к указанному снимку; разрешение публикации не предоставляется.'),'sc-form-status'));
}

function exactInspectionAction(section,selected,exact){
  if(!exact)return;
  const action=el('button',ui('Показать полную форму'),'sc-form-inspect');action.type='button';
  action.addEventListener('click',()=>{
    action.remove();section.dataset.exactFormInspected='true';
    section.append(el('p',ui('Показан полный пакет по точной ссылке. Предел доставки не меняет состояние формы и не означает её семантического принятия.'),'sc-form-status'));
    appendReadyPacket(section,exact.packet,selected.role,`form:${selected.role}:exact`);
  });
  section.append(action);
}

export function renderHumanForms(raw,{exactForms=null}={}){
  const view=formView(raw),container=el('div','','sc-human-forms');
  if(!view){container.append(el('p',ui('Этот ответ не содержит выбранных форм.'),'sc-reader-language-note'));return container;}
  const inspected=exactForms||inspectExactHumanForms(raw);
  container.dataset.formState=view.selection.state;
  if(view.selection.state!=='available')container.append(el('p',ui(stateLabels[view.selection.state]),'sc-form-status'));
  container.append(el('p',ui('Формы переданы источником. Доступность не означает семантического принятия.'),'sc-form-status'));
  if(view.selection.source_ref)container.append(el('p',view.selection.source_ref,'sc-source-ref'));
  for(const selected of view.roles){
    const section=el('section','','sc-form-role');section.dataset.formRole=selected.role;section.dataset.formState=selected.state;
    const heading=el('h5',ui(roleLabels[selected.role]));heading.dataset.readingAnchor='form:'+selected.role+':heading';section.append(heading);
    if(selected.state!=='ready'){
      section.append(el('p',ui(stateLabels[selected.state]),'sc-form-status'));
      section.append(el('p',ui(reasonLabels[selected.reason]),'sc-reader-language-note'));
      if(selected.state==='over-budget'&&selected.reason==='inspect-exact-form'){
        exactInspectionAction(section,selected,inspected?.[selected.role]);
        if(!inspected?.[selected.role])section.append(el('p',ui('Полную форму не удалось проверить в этой версии ответа.'),'sc-form-status'));
      }
    }else{
      const packet=selected.packet;section.dataset.formId=packet.form.id;section.dataset.formDigest=packet.form.digest;
      section.append(el('p',ui(reasonLabels[selected.reason]),'sc-reader-language-note'));
      appendReadyPacket(section,packet,selected.role);
    }
    const diagnostic=selected.candidates.filter(candidate=>candidate.state!=='ready');
    for(const candidate of diagnostic)section.append(el('p',ui('{0} · {1}',[ui(stateLabels[candidate.state]),candidate.form.id]),'sc-form-status'));
    if(selected.state==='ambiguous')for(const candidate of selected.candidates)section.append(el('p',candidate.form.id+' · '+(candidate.language||String(ui('Не указан'))),'sc-form-status'));
    container.append(section);
  }
  for(const issue of view.selection.issues)container.append(el('p',issue,'sc-form-status'));
  for(const candidate of view.selection.candidates.filter(value=>value.role===null)){
    container.append(el('p',ui('{0} · {1}',[ui(stateLabels[candidate.state]),candidate.form.id]),'sc-form-status'));
    container.append(jsonBlock(candidate,'unassigned-form'));
  }
  return container;
}
export function renderClaimContext(resolved){
  const section=el('section','','sc-form-context sc-claim-context');section.append(el('h5',ui('Контекст чтения утверждения')));
  const reading=resolved?.reading||{};
  section.append(el('p',ui('Тип чтения: {0}',[reading.mode||ui('не указан')]),'sc-reader-language-note'));
  const contexts=resolved?.semantics?.assertion_contexts;
  if(Array.isArray(contexts)&&contexts.length){
    section.append(el('h6',ui('Контексты утверждения')));
    contexts.forEach((context,index)=>{
      const item=el('section','','sc-assertion-context');item.dataset.contextIndex=String(index);
      item.append(el('p',ui('Контекст {0}',[index+1]),'sc-form-context-slot'));
      const fields=context&&typeof context==='object'&&context.fields&&typeof context.fields==='object'?context.fields:{};
      const values=el('dl','','sc-form-values');
      for(const [name,field] of Object.entries(fields)){
        const row=el('div');row.append(el('dt',name,'sc-form-value-key'));
        const value=field&&typeof field==='object'&&Object.hasOwn(field,'value')?field.value:field;
        row.append(el('dd','','sc-form-value'));row.lastChild.append(readableValue(value,`claim-context:assertion:${index}:${name}`));
        if(typeof field?.source_pointer==='string')row.append(el('small',field.source_pointer,'sc-source-ref'));
        values.append(row);
      }
      if(!values.children.length)values.append(el('dd',ui('Поля контекста не предоставлены.'),'sc-reader-gap'));
      item.append(values);
      if(Array.isArray(context?.conflicts)&&context.conflicts.length)item.append(el('p',ui('Конфликты: {0}',[context.conflicts.join(', ')]),'sc-form-status'));
      if(typeof context?.interpretation==='string')item.append(el('p',context.interpretation,'sc-form-status'));
      item.append(exactDetails(ui('Точная запись контекста утверждения'),context,`claim-context:assertion:${index}:raw`));
      section.append(item);
    });
  }else section.append(el('p',ui('Контекст утверждения не предоставлен.'),'sc-reader-gap'));
  const relations=Array.isArray(resolved?.relations)?resolved.relations:[];
  if(relations.length){
    section.append(el('h6',ui('Связи обязательного контекста')));
    relations.forEach((relation,index)=>{
      const item=el('section','','sc-assertion-context');
      item.append(el('p',relation.relation_type_id||relation.predicate_id||ui('Связь без типа'),'sc-form-context-slot'));
      item.append(el('p',ui('{0} → {1}',[relation.from_id||ui('не указан'),relation.to_id||ui('не указан')]),'sc-form-status'));
      item.append(exactDetails(ui('Точная запись связи'),relation,`claim-context:relation:${index}:raw`));section.append(item);
    });
  }
  section.append(exactDetails(ui('Точные данные чтения'),resolved,'claim-context:raw'));
  return section;
}
export function renderEssentialContext(context){
  const section=el('section','','sc-form-role sc-record-context');section.dataset.contextState=context.state;
  section.hidden=context.state==='not-declared'||context.state==='available'&&!context.items.length;
  if(section.hidden)return section;
  section.append(el('h5',ui('Обязательный контекст записи')));
  if(context.state==='unavailable')section.append(el('p',ui('Объявленный контекст недоступен в этой версии ответа.'),'sc-reader-gap'));
  context.items.forEach((item,index)=>{
    const entry=el('section','','sc-form-context');entry.dataset.contextPointer=typeof item.pointer==='string'?item.pointer:'';
    entry.append(el('h6',ui('Контекст {0}',[index+1])));
    if(item.state==='available'){
      entry.append(renderContextData(item.value,'record-context:'+index));
    }
    else entry.append(el('p',ui('Объявленный контекст недоступен в этой версии ответа.'),'sc-reader-gap'));
    if(typeof item.pointer==='string'){
      const route=el('details','','sc-context-location');route.dataset.readingKey='record-context:'+index+':route';
      route.append(el('summary',ui('Расположение в ответе')),el('p',item.pointer,'sc-source-ref'));entry.append(route);
    }
    section.append(entry);
  });
  if(['unavailable','incomplete'].includes(context.state))section.append(el('p',ui('Чтобы проверить отсутствующий контекст, откройте источники материала.'),'sc-form-status'));
  return section;
}
