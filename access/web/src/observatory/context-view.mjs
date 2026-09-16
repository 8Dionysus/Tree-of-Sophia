import {ui,uiText} from './ui-i18n.mjs';
import {languageName} from './human-presentation.mjs';
const el=(tag,text='',className='')=>{const node=document.createElement(tag);node.className=className;uiText(node,text);return node;};
const labels={title:'Название',preferred_label:'Название',variant_labels:'Другие названия',language:'Язык',
  notes:'Примечания',note:'Примечание',context:'Контекст',scope:'Область действия',constraints:'Ограничения',
  conflicts:'Расхождения',interpretation:'Интерпретация',qualification:'Уточнение',qualifiers:'Уточнения',
  uncertain:'Неопределённость',negation:'Отрицание',confidence:'Уверенность',polarity:'Характер утверждения',
  modality:'Модальность',attribution:'Авторство',date:'Дата',year:'Год',place:'Место',publisher:'Издатель',
  author:'Автор',description:'Описание',summary:'Описание',identity_status:'Идентификация',same_as_posture:'Отождествление',
  text:'Текст',statement:'Утверждение',scope_note:'Область описания',identity_criterion:'Критерий идентичности',
  system_account:'Описание',coverage_and_loss:'Сохранность',distinguishing_basis:'Отличительные признаки',
  mapping_convention:'Соответствие',script_account:'Письменность',sign_inventory_scope:'Состав знаков',
  assertion_force:'Характер утверждения',challenge_account:'Возражения',proposition:'Положение',reasoning_mode:'Ход рассуждения',
  reconstruction_note:'Реконструкция',transition_account:'Переход',relation_basis:'Основание связи',
  community_practice:'Практика сообщества',group_account:'Описание группы',institutional_account:'Устройство организации',
  membership_boundary:'Состав сообщества',time_scope_note:'Временные рамки'};
const values={verified:'Проверено',unverified:'Не проверено',disputed:'Оспаривается',unknown:'Неизвестно',
  no_equivalence_claim:'Отождествление не заявлено',affirmative:'Утверждение',negative:'Отрицание',uncertain:'Неопределённо'};
const wrappers=new Set(['fields','record','value','source_record','source_claim','semantic_scope','semantic_content','qualifiers']);

// Fallback uses named human fields. Exact source data is retained by the caller
// for export; transport structures are never serialized into ordinary reading.
export function renderContextData(value,anchor){
  const container=el('div','','sc-context-data');let position=0;
  function scalar(item,key){
    if(item===null||item===undefined||item==='')return null;
    if(typeof item==='boolean')return item?ui('Да'):ui('Нет');
    if(typeof item==='number')return String(item);
    if(typeof item==='string')return key==='language'?languageName(item):Object.hasOwn(values,item)?ui(values[item]):item;
    return null;
  }
  function append(parent,item,key='',depth=0){
    if(depth>12)return;
    const text=scalar(item,key);
    if(text!==null){const node=el('p',text,'sc-context-value');node.dir='auto';node.dataset.readingAnchor=position?anchor+':part:'+position:anchor;position++;parent.append(node);return;}
    if(!item||typeof item!=='object')return;
    if(Array.isArray(item)){for(const child of item)append(parent,child,key,depth+1);return;}
    if(Object.hasOwn(item,'value')){append(parent,item.value,key,depth+1);return;}
    const list=el('dl','','sc-context-fields');
    const language=typeof item.language==='string'?item.language:null;
    const prose=Object.keys(item).some(field=>field!=='language'&&Object.hasOwn(labels,field)&&typeof item[field]==='string');
    for(const [field,child]of Object.entries(item)){
      if(field==='language'&&prose)continue;
      if(wrappers.has(field)){append(list,child,field,depth+1);continue;}
      if(!Object.hasOwn(labels,field))continue;
      const definition=el('dd');if(language)definition.lang=language;append(definition,child,field,depth+1);if(!definition.children.length)continue;
      if(field==='identity_criterion'){
        const detail=el('details');detail.append(el('summary',ui(labels[field])),definition);list.append(detail);continue;
      }
      const row=el('div','','sc-context-field');row.append(el('dt',ui(labels[field])),definition);list.append(row);
    }
    if(list.children.length)parent.append(list);
  }
  append(container,value);return container;
}
