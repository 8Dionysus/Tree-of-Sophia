import {createSourceCommandClient} from './source-command-client.mjs';
import {createSourceFormSession} from './source-form-session.mjs';
import './source-command-panel.css';

const words={ru:{
  scope:'Отдельное подключение к владельцу источника. Сейчас здесь доступно создание и обновление точных копий исходных полей; это не новая интерпретация и не оценка.',
  target:'Источник определён отдельной делегацией, а не выбранной звездой. Проверьте предмет перед изменением.',
  origin:'Адрес локального владельца',token:'Приватный ключ подключения',connect:'Подключить',
  field:'Исходное поле',form:'Разрешённая форма',prepare:'Подготовить изменение',apply:'Применить точную копию',
  preview:'Предварительный просмотр',technical:'Точные версии и основания',copy:'Скопировать сохранённую команду',
  noPreview:'Владелец ещё не вернул человекочитаемый просмотр этой подготовленной формы. Применение из этого окна недоступно.',
  uncertain:'Исход операции не подтверждён. Источник мог измениться. Сохраните точную команду; повтор возможен только с тем же ID и содержимым.',
  retry:'Проверить повтором той же команды',done:'Владелец подтвердил запись. Читатель и сцена не обновлены автоматически; это отдельная публикация, не допуск знания.',
  copied:'Точная команда скопирована. Она может содержать приватный материал: храните её соответственно.',
  missing:'Выбранный владелец не предоставляет этот рабочий путь.',name:'Имя',hover:'Примечание',statement:'Формулировка',
  caption:'Подпись',unknown:'Язык не указан',connecting:'Подключение…',working:'Обращение к владельцу…',
},en:{
  scope:'A separate source-owner connection. This workflow creates or revises exact source-field copies, not interpretations or assessments.',
  target:'The separate delegation selects the source, not the selected star. Verify the target before applying a change.',
  origin:'Local owner origin',token:'Private connection credential',connect:'Connect',field:'Source field',form:'Delegated form',
  prepare:'Prepare change',apply:'Apply exact copy',preview:'Preview',technical:'Exact versions and grounds',copy:'Copy retained command',
  noPreview:'The owner has not supplied a readable preview of this prepared form. Applying it from this panel is unavailable.',
  uncertain:'The outcome is unconfirmed. The source may have changed. Retain the exact command; replay must keep the same ID and content.',
  retry:'Reconcile using the same command',done:'The owner confirmed the write. Reader and scene were not republished automatically; this is not knowledge admission.',
  copied:'The exact command was copied. It may contain private material; retain it accordingly.',missing:'This owner does not provide this workflow.',
  name:'Name',hover:'Note',statement:'Statement',caption:'Caption',unknown:'Language unspecified',connecting:'Connecting…',working:'Contacting source owner…',
}};
const el=(tag,text='',className='')=>{const node=document.createElement(tag);node.textContent=text;node.className=className;return node;};
const action=(text,run)=>{const node=el('button',text,'text-button');node.type='button';node.onclick=run;return node;};
function details(label,value){const node=el('details');node.append(el('summary',label),el('pre',JSON.stringify(value,null,2)));return node;}

export function mountSourceCommandPanel(container,{language='ru',onChanged=()=>{}}={}){
  const t=words[language]??words.en;
  let client=null,session=null,disposed=false;
  const status=el('p','','muted');status.setAttribute('role','status');
  const content=el('div','','source-command-panel');container.replaceChildren(el('p',t.scope,'body'),status,content);
  const showError=error=>{if(!disposed)status.textContent=error?.code??error?.message??t.missing;};
  async function perform(button,operation){button.disabled=true;status.textContent=t.working;
    try{await operation();}catch(error){showError(error);}finally{if(!disposed)button.disabled=false;}}
  async function commit(){
    // commit() retains the exact request synchronously, before its first await.
    // Install the unload guard now, not only after receiving a successful reply.
    const operation=session.commit();onChanged();
    try{render(await operation);}finally{render(session.state());onChanged();}
  }

  function render(state){
    if(disposed)return;
    content.replaceChildren();status.textContent='';
    const current=state.current;
    const name=current.materializations?.find(item=>item.state==='ready'&&item.role==='name'&&typeof item.display_text==='string');
    content.append(el('p',t.target,'body'),el('h3',name?.display_text??current.source.id,'minor-title'));
    content.append(details(t.technical,{source:current.source,owner_configuration:current.owner_configuration,
      revision:current.revision,allowed_operations:current.allowed_operations}));
    if(state.pending){
      content.append(el('p',state.uncertain?t.uncertain:t.done,'body'));
      const copy=action(t.copy,()=>void perform(copy,async()=>{
        await navigator.clipboard.writeText(JSON.stringify(session.retainedCommand(),null,2));status.textContent=t.copied;}));
      content.append(copy,details(t.technical,state.result??state.pending));
      if(state.uncertain){const retry=action(t.retry,()=>void perform(retry,commit));content.append(retry);}
      return;
    }
    if(state.prepared){
      const preview=current.prepared_materialization;
      content.append(el('h3',t.preview,'minor-title'));
      const ready=preview?.state==='ready'&&typeof preview.display_text==='string'&&
        preview.form?.id===state.prepared.form.form_id&&
        preview.form?.version===state.prepared.form.form_version;
      if(ready){content.append(el('p',preview.display_text,'body'),details(t.technical,preview.context));}
      else content.append(el('p',t.noPreview,'body'));
      content.append(details(t.technical,state.prepared));
      const apply=action(t.apply,()=>void perform(apply,commit));
      apply.disabled=!ready;content.append(apply);
      return;
    }
    const fields=el('select'),forms=el('select');
    fields.setAttribute('aria-label',t.field);forms.setAttribute('aria-label',t.form);
    for(const field of current.source_fields){const option=el('option',`${t[field.role]??field.role} · ${field.language??t.unknown} · ${field.field_id}`);
      option.value=field.field_id;fields.append(option);}
    for(const id of current.allowed_form_ids){const option=el('option',id);option.value=id;forms.append(option);}
    const fieldLabel=el('label',t.field),formLabel=el('label',t.form);fieldLabel.append(fields);formLabel.append(forms);
    const prepare=action(t.prepare,()=>void perform(prepare,async()=>render(await session.prepare({formId:forms.value,fieldId:fields.value}))));
    prepare.disabled=!fields.options.length||!forms.options.length;content.append(fieldLabel,formLabel,prepare);
  }

  const connection=el('form'),origin=el('input'),credential=el('input');
  origin.type='url';origin.required=true;origin.placeholder='http://127.0.0.1:44259';origin.autocomplete='off';
  credential.type='password';credential.required=true;credential.autocomplete='off';credential.maxLength=64;
  const originLabel=el('label',t.origin),tokenLabel=el('label',t.token);originLabel.append(origin);tokenLabel.append(credential);
  const connect=el('button',t.connect,'primary');connect.type='submit';connection.append(originLabel,tokenLabel,connect);content.append(connection);
  connection.onsubmit=event=>{event.preventDefault();void perform(connect,async()=>{
    client?.close();client=createSourceCommandClient({origin:origin.value,token:credential.value});credential.value='';
    session=createSourceFormSession(client);render(await session.describe());
  });};
  return {dispose(){disposed=true;credential.value='';client?.close();},
    hasUnconfirmedCommand:()=>{const state=session?.state();return Boolean(state?.pending&&
      (state.busy||state.uncertain||!state.result?.receipt));},
    retainedCommand:()=>session?.retainedCommand()??null};
}
