import {MAX_CONDITIONS,conditionCatalog,conditionKey,conditionText,defaultCondition,operatorLabels} from './lens-conditions.mjs';

const el=(tag,text='',className='')=>{const node=document.createElement(tag);node.textContent=text;node.className=className;return node;};
const option=(value,text)=>{const node=el('option',text);node.value=value;return node;};
const button=(text,action)=>{const node=el('button',text,'sc-builder-link');node.type='button';node.addEventListener('click',action);return node;};
const field=(text,input)=>{const label=el('label','','sc-builder-field');label.append(el('span',text),input);return label;};
const number=text=>/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?$/i.test(text.trim())&&Number.isFinite(Number(text))?Number(text):text;
let listId=0;
export function createConditionEditor({draft,context,kind,onChange}){
  const entries=conditionCatalog(context,kind),rules=draft.conditions[kind],isNode=kind==='nodes';
  const section=el('section','','sc-conditions');section.setAttribute('aria-label',isNode?'Условия исходных узлов':'Условия связей');
  section.append(el('h4',isNode?'Условия исходных узлов':'Условия связей'));
  const dormant=isNode?draft.scope==='focus':!draft.relations;
  section.append(el('p',dormant?(isNode?'Сохранены, но не действуют при выборе явного центра.':'Сохранены, но не действуют, пока связи выключены.'):'Все условия этого раздела действуют одновременно.','sc-builder-note'));
  if(!entries.length)section.append(el('p','Сервер пока не объявил совместимые свойства и операции.','sc-builder-note'));
  function renderRows(){
    section.querySelectorAll('.sc-condition,.sc-condition-add').forEach(node=>node.remove());
    rules.forEach((rule,index)=>{
      const row=el('fieldset','','sc-condition');row.append(el('legend',`Условие ${index+1}`));
      const controls=el('div','','sc-condition-controls');
      const search=el('input');search.type='search';search.placeholder='Найти свойство…';search.setAttribute('aria-label',`Найти свойство условия ${index+1}`);
      const selector=el('select');selector.setAttribute('aria-label',`Свойство условия ${index+1}`);
      const details=el('div','','sc-condition-detail');
      function choices(){
        const needle=search.value.toLocaleLowerCase('ru'),current=conditionKey(rule),chosen=entries.find(e=>conditionKey(e)===current);
        selector.replaceChildren();
        const filtered=entries.filter(e=>(e.title+' '+e.id).toLocaleLowerCase('ru').includes(needle));
        const visible=filtered.slice(0,100);if(chosen&&!visible.includes(chosen))visible.unshift(chosen);
        if(!chosen)selector.append(option(current,'Недоступно: '+rule.id));
        for(const [key,title]of [['property_id','Свойства сущностей'],['field','Поля представления']]){
          const group=el('optgroup');group.label=title;
          const titles=new Map();for(const e of visible)titles.set(e.title,(titles.get(e.title)||0)+1);
          for(const e of visible.filter(e=>e.selector===key))group.append(option(conditionKey(e),e.title+(titles.get(e.title)>1?' · '+e.id:'')));
          if(group.children.length)selector.append(group);
        }
        selector.value=current;
        if(filtered.length>100){const more=option('','Показаны первые 100 — уточните поиск');more.disabled=true;selector.append(more);}
      }
      function renderValue(){
        details.replaceChildren();const entry=entries.find(e=>conditionKey(e)===conditionKey(rule));
        if(!entry){details.append(el('p',conditionText(rule,[]),'sc-builder-note'),el('p','Это условие сохранено. Выберите доступное свойство или удалите условие.','sc-builder-warning'));return;}
        const op=el('select');op.setAttribute('aria-label',`Операция условия ${index+1}`);
        if(!entry.operators.includes(rule.op))op.append(option(rule.op,'Недоступно: '+(operatorLabels[rule.op]||rule.op)));
        for(const id of entry.operators)op.append(option(id,operatorLabels[id]));op.value=rule.op;
        op.addEventListener('change',()=>{
          rule.op=op.value;
          if(rule.op==='exists'||entry.valueType==='boolean')rule.value=true;
          else if(Array.isArray(rule.value)||typeof rule.value==='boolean')rule.value='';
          renderValue();onChange();details.querySelector('input,textarea,select:last-child')?.focus();
        });
        const boolean=rule.op==='exists'||entry.valueType==='boolean',multi=rule.op==='in'||rule.op==='contains'&&entry.valueType==='string-array';
        let input;
        if(boolean){input=el('select');input.append(option('true','Да'),option('false','Нет'));input.value=String(rule.value);}
        else if(multi){input=el('textarea');input.rows=2;input.placeholder='По одному значению на строку';input.value=Array.isArray(rule.value)?rule.value.join('\n'):String(rule.value);input.maxLength=12000;}
        else{input=el('input');input.type='text';input.maxLength=1024;input.value=String(rule.value);if(entry.valueType==='number')input.inputMode='decimal';}
        input.setAttribute('aria-label',`Значение условия ${index+1}`);
        const update=(event={})=>{
          rule.value=boolean?input.value==='true':multi?input.value.split('\n').map(v=>entry.valueType==='number'?number(v):v):entry.valueType==='number'?number(input.value):input.value;
          onChange({composing:event.isComposing===true});
        };
        input.addEventListener(boolean?'change':'input',update);
        input.addEventListener('compositionstart',()=>onChange({composing:true}));
        input.addEventListener('compositionend',()=>update());
        const pair=el('div','','sc-condition-value');pair.append(field('Операция',op),field(boolean?'Выберите значение':multi?'Значения · по одному на строку':entry.valueType==='number'?'Число':'Значение',input));details.append(pair);
        if(entry.suggestions.length&&!boolean&&!multi){
          const values=el('datalist');values.id='sc-condition-values-'+(++listId);
          for(const value of entry.suggestions.slice(0,100))values.append(option(String(value),String(value)));
          input.setAttribute('list',values.id);details.append(values);
        }
        if(entry.selector==='property_id'){
          const info=el('details','','sc-condition-definition');info.append(el('summary','Смысл и область свойства'));
          if(entry.definition)info.append(el('p',entry.definition));
          const types=context.catalog.semantic_registries?.entity_types?.entries||[];
          info.append(el('p','Применимо к: '+entry.appliesTo.map(id=>{const t=types.find(t=>t.type_id===id);return t?.labels?.ru||t?.labels?.default||id;}).join(', ')+(entry.inherited?' и их подтипам.':'.')));
          if(entry.unit)info.append(el('p','Единица: '+entry.unit));if(entry.language)info.append(el('p','Язык значения: '+entry.language));
          if(entry.valueType.startsWith('string'))info.append(el('p','Значение передаётся без изменения регистра, языка или символов. Правила сравнения задаёт сервер.'));
          info.append(el('code',entry.id));details.append(info);
        }
      }
      search.addEventListener('input',choices);
      selector.addEventListener('change',()=>{const entry=entries.find(e=>conditionKey(e)===selector.value);if(entry){Object.assign(rule,defaultCondition(entry));renderValue();onChange();}});
      choices();renderValue();controls.append(search,selector);row.append(controls,details,button('Удалить условие '+(index+1),()=>{rules.splice(index,1);renderRows();onChange();section.querySelector('.sc-condition-add')?.focus();}));section.append(row);
    });
    const add=button(isNode?'＋ Условие узла':'＋ Условие связи',()=>{rules.push(defaultCondition(entries[0]));renderRows();onChange();section.querySelector('.sc-condition:last-of-type input')?.focus();});
    add.classList.add('sc-condition-add');add.disabled=dormant||!entries.length||rules.length>=MAX_CONDITIONS;section.append(add);
  }
  renderRows();return section;
}
