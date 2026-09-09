import {ui,uiText} from './ui-i18n.mjs';

const el=(tag,text='',className='')=>{const node=document.createElement(tag);node.className=className;uiText(node,text);return node;};
// Labels explain field names only. Values and their order stay source-owned;
// unknown fields remain visible under their exact key.
const labels={source_ref:'Источник',source_refs:'Источники',source_pointer:'Поле источника',
  record:'Запись',record_id:'Идентификатор записи',record_type:'Тип записи',record_version:'Версия записи',
  record_ref:'Ссылка на запись',fields:'Поля записи',value:'Содержание',notes:'Примечания',
  title:'Название',preferred_label:'Основное название',variant_labels:'Варианты названия',
  language:'Язык',language_context:'Языковое происхождение',context:'Контекст',
  relations:'Связи',constraints:'Ограничения',scope:'Область действия',conflicts:'Расхождения',
  status:'Статус',reason:'Основание',interpretation:'Интерпретация',source_record_digest:'Контрольная сумма записи',
  schema_version:'Версия схемы',binding_role:'Роль привязки',supersedes_ref:'Предыдущая версия',
  provenance:'Происхождение',external_identifiers:'Внешние идентификаторы',identity_status:'Статус идентификации',
  same_as_posture:'Статус отождествления'};

export function renderContextData(value,anchor){
  let position=0;
  function leaf(text,type){
    const node=el('p',text,'sc-context-value');node.dataset.valueType=type;node.dir='auto';
    node.dataset.readingAnchor=position?anchor+':part:'+position:anchor;position++;return node;
  }
  function render(item,depth=0){
    if(item===null)return leaf(ui('Значение null'),'null');
    if(typeof item==='string')return leaf(item===''?ui('Пустая строка'):item,'string');
    if(typeof item==='boolean')return leaf(item?ui('Да (true)'):ui('Нет (false)'),'boolean');
    if(typeof item==='number')return leaf(String(item),'number');
    const entries=Object.entries(item);
    if(!entries.length)return leaf(Array.isArray(item)?ui('Пустой список'):ui('Пустой объект'),Array.isArray(item)?'array':'object');
    // Exceptionally deep data stays complete without unbounded DOM recursion.
    if(depth>=16)return el('pre',JSON.stringify(item,null,2),'sc-form-json');
    if(Array.isArray(item)){
      const list=el('ol','','sc-context-list');for(const child of item){const row=el('li');row.append(render(child,depth+1));list.append(row);}return list;
    }
    const list=el('dl','','sc-context-fields');
    for(const [key,child] of entries){
      const row=el('div','','sc-context-field'),term=el('dt',Object.hasOwn(labels,key)?ui(labels[key]):key);
      term.dataset.sourceKey=key;const definition=el('dd');definition.append(render(child,depth+1));
      row.append(term,definition);list.append(row);
    }
    return list;
  }
  const container=el('div','','sc-context-data');container.append(render(value));
  const exact=el('details','','sc-context-exact');exact.dataset.readingKey=anchor+':exact';
  exact.append(el('summary',ui('Точные данные контекста')));
  const json=el('pre',JSON.stringify(value,null,2),'sc-form-json');json.dir='auto';exact.append(json);container.append(exact);
  return container;
}
