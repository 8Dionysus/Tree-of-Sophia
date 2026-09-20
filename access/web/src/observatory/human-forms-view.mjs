import {ui,uiText,uiComputed,uiLanguage} from './ui-i18n.mjs';
import {formView,inspectExactHumanForms} from './human-forms.mjs';
import {renderContextData} from './context-view.mjs';
import {formContexts} from './readable-context.mjs';
import {renderReadableContexts} from './readable-context-view.mjs';
const el=(tag,value='',className='')=>{const node=document.createElement(tag);node.className=className;uiText(node,value);return node;};
const disclosure=(title,group)=>{const node=el('details','','sc-form-disclosure');node.dataset.formGroup=group;node.append(el('summary',title));return node;};
const usable=role=>role&&(role.state==='ready'||role.state==='over-budget'&&role.reason==='inspect-exact-form');
function packetContext(packet,anchor,readableContext,presentation={}){
  const contexts=formContexts(readableContext,packet.form);
  if(readableContext?.state==='complete'&&contexts.length)return renderReadableContexts(contexts,anchor,presentation);
  const context=el('section','','sc-form-context');
  if(readableContext&&readableContext.state!=='complete'){const gap=el('p',ui('Часть контекста доступна в источнике.'),'sc-reader-gap');gap.dataset.contextPresentation=readableContext.state;context.append(gap);}
  for(const [index,entry]of packet.context.entries()){
    const pointer=entry.binding?.pointer??'';if(pointer.startsWith('/field_languages/'))continue;
    const key=pointer.split('/').at(-1)?.replace(/~1/g,'/').replace(/~0/g,'~');
    const rendered=renderContextData(key?{[key]:entry.value}:entry.value,`${anchor}:${index}`);rendered.dataset.contextSlot=entry.slot;
    if(/^\/variant_labels\/\d+\/language$/.test(pointer)){
      const title=rendered.querySelector('dt');if(title)uiText(title,uiComputed(()=>({ru:'Язык названия',en:'Title language',es:'Idioma del título'}[uiLanguage()]??'Title language')));
    }
    if(rendered.children.length)context.append(rendered);
  }
  return context;
}
function appendPacket(section,packet,role,readableContext,presentation){
  const anchor=`form:${role}`;section.dataset.formId=packet.form.id;section.dataset.formDigest=packet.form.digest;
  const wording=el('div',packet.display_text,'sc-form-wording');wording.dir='auto';if(packet.language)wording.lang=packet.language;
  wording.dataset.readingAnchor=anchor+':wording';section.append(wording);
  const context=packetContext(packet,anchor+':context',readableContext,presentation);if(context.children.length)section.append(context);
}
function renderRole(selected,inspected,readableContext,presentation){
  const section=el('section','','sc-form-role');section.dataset.formRole=selected.role;section.dataset.formState=selected.state;
  if(selected.state==='ready')appendPacket(section,selected.packet,selected.role,readableContext,presentation);
  else if(inspected?.[selected.role]){
    const action=el('button',ui('Показать полностью'),'sc-form-inspect');action.type='button';
    action.addEventListener('click',()=>{action.remove();section.dataset.exactFormInspected='true';appendPacket(section,inspected[selected.role].packet,selected.role,readableContext,presentation);});section.append(action);
  }
  return section;
}
export function renderHumanForms(raw,{exactForms=null,readableContext=null,presentation={}}={}){
  const view=formView(raw),container=el('div','','sc-human-forms');if(!view)return container;
  container.dataset.formState=view.selection.state;
  const inspected=exactForms||inspectExactHumanForms(raw),roles=new Map(view.roles.map(role=>[role.role,role]));
  const primary=['statement','caption'].map(role=>roles.get(role)).find(usable),shown=new Set();
  const add=(selected,parent)=>{
    const packet=selected.packet??inspected?.[selected.role]?.packet;if(!packet)return;const identity=JSON.stringify([packet.display_text,packet.context]);if(shown.has(identity))return;
    shown.add(identity);const section=renderRole(selected,inspected,readableContext,presentation);if(section.children.length)parent.append(section);
  };
  if(primary)add(primary,container);
  const hover=roles.get('hover');
  if(!primary&&usable(hover)){
    const packet=hover.packet??inspected?.hover?.packet;
    if(packet?.derivation==='source-copy'){
      const text=disclosure(ui('Примечание источника'),'source-note');add(hover,text);if(text.children.length>1)container.append(text);
    }else add(hover,container);
  }
  for(const [role,title]of [['grounds','Основания'],['history','История']]){
    const selected=roles.get(role);if(!usable(selected))continue;
    const section=disclosure(ui(title),role);add(selected,section);if(section.children.length>1)container.append(section);
  }
  // Metadata has no statement: show its supplied facts once, independently of
  // the source note. Do not repeat names or empty roles as reading sections.
  if(!primary){
    const name=roles.get('name');const packet=name?.packet??hover?.packet;
    if(packet){const context=packetContext(packet,'form:name:context',readableContext,presentation);if(context.children.length)container.append(context);}
  }
  if(!container.children.length){
    const states=['statement','caption','hover'].map(role=>roles.get(role)?.state);
    const message=states.includes('ambiguous')?ui('Вариант текста не определён.'):states.includes('unavailable')?ui('Текст недоступен.'):ui('Текст пока не предоставлен.');
    container.append(el('p',message,'sc-reader-gap'));
  }
  return container;
}
export function renderClaimContext(resolved,readableContext=null,presentation={}){
  const section=el('section','','sc-form-context sc-claim-context');
  for(const [index,context]of (resolved?.semantics?.assertion_contexts??[]).entries()){
    const pointer=`/semantics/assertion_contexts/${index}`;
    if(presentation.presentedPointers?.has(pointer))continue;
    const classified=readableContext?.state==='complete'?readableContext.contexts.filter(value=>value.form===null&&value.origin_pointer===pointer):[];
    const entry=classified.length?renderReadableContexts(classified,`claim-context:${index}`,presentation):renderContextData(context,`claim-context:${index}`);
    if(entry.children.length)section.append(entry);
  }
  if(readableContext&&readableContext.state!=='complete'){const gap=el('p',ui('Часть контекста доступна в источнике.'),'sc-reader-gap');gap.dataset.contextPresentation=readableContext.state;section.append(gap);}
  return section;
}
export function renderEssentialContext(context,readableContext=null,presentation={}){
  const section=el('section','','sc-form-role sc-record-context');section.dataset.contextState=context.state;
  if(context.state==='not-declared')return section;
  for(const [index,item]of context.items.entries()){
    if(item.state!=='available')continue;
    const classified=readableContext?.state==='complete'?readableContext.contexts.filter(value=>value.form===null&&value.origin_pointer===item.pointer):[];
    const entry=classified.length?renderReadableContexts(classified,'record-context:'+index,presentation):renderContextData(item.value,'record-context:'+index);
    if(entry.children.length){
      section.append(entry);
      // The claim view can point to this same delivered context. Track its
      // exact pointer so a reading shows that context once across both views.
      (presentation.presentedPointers??=new Set()).add(item.pointer);
    }
  }
  if(readableContext&&readableContext.state!=='complete'){const gap=el('p',ui('Часть контекста доступна в источнике.'),'sc-reader-gap');gap.dataset.contextPresentation=readableContext.state;section.append(gap);}
  if(['unavailable','incomplete'].includes(context.state))section.append(el('p',ui('Часть сведений недоступна.'),'sc-reader-gap'));
  return section;
}
