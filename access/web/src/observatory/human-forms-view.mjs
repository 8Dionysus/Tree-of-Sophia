import {ui,uiText} from './ui-i18n.mjs';
import {formView,inspectExactHumanForms} from './human-forms.mjs';

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

function appendReadyPacket(section,packet,role,anchorPrefix=`form:${role}`){
  section.append(el('p',ui('Язык формы: {0}',[packet.language||ui('Не указан')]),'sc-reader-language-note'));
  if(packet.derivation==='source-copy')section.append(el('p',ui('Полная копия поля источника; истинность формулировки не оценивалась.'),'sc-form-status'));
  const wording=el('div',packet.display_text,'sc-form-wording');wording.dir='auto';if(packet.language)wording.lang=packet.language;
  wording.dataset.readingAnchor=anchorPrefix+':wording';section.append(wording);
  if(packet.context.length){
    section.append(el('h6',ui('Обязательный контекст')));
    packet.context.forEach((entry,index)=>{
      const context=el('section','','sc-form-context');context.dataset.contextSlot=entry.slot;
      context.append(jsonBlock(entry.value,anchorPrefix+':context:'+index));
      // Bindings and unknown context members are retained alongside value.
      const {value,...binding}=entry;context.append(jsonBlock(binding,anchorPrefix+':binding:'+index));section.append(context);
    });
  }
  if(packet.language_context){section.append(el('h6',ui('Языковое происхождение')),jsonBlock(packet.language_context,anchorPrefix+':language'));}
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
  const section=el('section','','sc-form-context');section.append(el('h5',ui('Контекст чтения утверждения')));
  section.append(jsonBlock(resolved,'claim-context'));return section;
}
export function renderEssentialContext(context){
  const section=el('section','','sc-form-role sc-record-context');section.dataset.contextState=context.state;
  section.hidden=context.state==='not-declared'||context.state==='available'&&!context.items.length;
  if(section.hidden)return section;
  section.append(el('h5',ui('Обязательный контекст записи')));
  if(context.state==='unavailable')section.append(el('p',ui('Объявленный контекст недоступен в этой версии ответа.'),'sc-reader-gap'));
  context.items.forEach((item,index)=>{
    const entry=el('section','','sc-form-context');entry.dataset.contextPointer=typeof item.pointer==='string'?item.pointer:'';
    if(typeof item.pointer==='string')entry.append(el('p',item.pointer,'sc-source-ref'));
    if(item.state==='available')entry.append(jsonBlock(item.value,'record-context:'+index));
    else entry.append(el('p',ui('Объявленный контекст недоступен в этой версии ответа.'),'sc-reader-gap'));
    section.append(entry);
  });
  return section;
}
