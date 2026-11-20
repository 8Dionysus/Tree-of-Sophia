// The field (L2-L4): room for what agents invent. An agent declares a kind of its own (x.<namespace>.<name>) and may
// give it one of the shapes the interface draws (contracts/field-shapes.v1.json): thoughts and links of the field,
// trial lenses that stand out on the sky, routes walked by the stand's own walker. A kind without a shape still shows
// by its declaration and its acts. Everything here is an agent's proposal in the person's journal; it becomes a
// ticket only by the person's explicit act, and nothing reaches the stand, its bonsai or the Tree.
import './act-desk.css';
import './agent-field.css';
import {localeText,localeValue,resolveLocale} from './locale.mjs';
import shapes from '../../../contracts/field-shapes.v1.json' with {type:'json'};

export const FIELD_SHAPES=Object.freeze(shapes.shapes.map(shape=>shape.name));
const MAX_MEMBERS=400,MAX_GROUPS=24,MAX_STEPS=40;
const text=(value,limit=280)=>typeof value==='string'?value.trim().slice(0,limit):'';
const address=value=>typeof value==='string'&&value.trim()&&value.length<=1024?value.trim():null;

/** The kinds agents declared, newest declaration of a name first wins: {name, description, shape, author, act}. */
export function declaredKinds(records){
  const kinds=new Map();
  for(const record of records){
    if(record.kind!=='kind-declaration')continue;
    const declares=record.body?.declares;if(!declares?.name||kinds.has(declares.name))continue;
    kinds.set(declares.name,{name:declares.name,description:text(declares.description,500),shape:FIELD_SHAPES.includes(declares.shape)?declares.shape:null,author:record.author,act:record.id});
  }
  return kinds;
}

/** One act of a shaped kind as the field draws it, or null when its body does not have the shape it declares. */
export function fieldItem(record,shape){
  const body=record.body??{},base={id:record.id,kind:record.kind,shape,author:record.author,at:record.at,targets:record.targets??[],grounds:record.grounds??[],intent:record.intent};
  if(shape==='thought'){const title=text(body.title),said=text(body.text,4000);return title&&said?{...base,title,text:said}:null;}
  if(shape==='link'){const from=address(body.from),to=address(body.to),verb=text(body.verb);return from&&to&&verb&&from!==to?{...base,from,to,verb,text:text(body.text,4000)}:null;}
  if(shape==='lens'){
    const members=[...new Set((Array.isArray(body.members)?body.members:[]).map(address).filter(Boolean))].slice(0,MAX_MEMBERS),title=text(body.title);
    const groups=(Array.isArray(body.groups)?body.groups:[]).slice(0,MAX_GROUPS).map(group=>({title:text(group?.title),members:(Array.isArray(group?.members)?group.members:[]).map(address).filter(Boolean)})).filter(group=>group.title&&group.members.length);
    return title&&members.length?{...base,title,description:text(body.description,1000),members,groups}:null;
  }
  if(shape==='route'){
    const steps=(Array.isArray(body.steps)?body.steps:[]).slice(0,MAX_STEPS).map(step=>({target:address(step?.target),title:text(step?.title),text:text(step?.text,2000)})).filter(step=>step.target);
    const title=text(body.title);
    return title&&steps.length>=2?{...base,title,question:text(body.question,500),steps}:null;
  }
  return null;
}

/** The field of a journal: declared kinds with their counts, the shaped items that are not withdrawn, the rest by intent. */
export function fieldOf(records){
  const kinds=declaredKinds(records),withdrawn=new Set();
  for(const record of records)if(record.kind==='withdrawal')for(const target of record.targets??[])if(target.startsWith('act:'))withdrawn.add(target.slice(4));
  const items=[],others=[],counts=new Map();
  for(const record of records){
    if(!record.kind?.startsWith('x.')||withdrawn.has(record.id))continue;
    counts.set(record.kind,(counts.get(record.kind)??0)+1);
    const kind=kinds.get(record.kind),item=kind?.shape?fieldItem(record,kind.shape):null;
    if(item)items.push(item);else others.push({id:record.id,kind:record.kind,author:record.author,intent:record.intent,targets:record.targets??[]});
  }
  return {kinds:[...kinds.values()].map(kind=>({...kind,acts:counts.get(kind.name)??0})),items,others,undeclared:[...counts.keys()].filter(name=>!kinds.has(name))};
}

/** A route of the field as the stand's walker walks it. */
export const routeForWalker=(item,{nameOf=id=>id}={})=>({id:`agent:${item.id}`,title:{ru:item.title,en:item.title},question:item.question?{ru:item.question,en:item.question}:null,
  byAgent:[item.author?.id,item.author?.model].filter(Boolean).join(' · '),
  steps:item.steps.map(step=>({nodeId:step.target,title:{ru:step.title||nameOf(step.target),en:step.title||nameOf(step.target)},body:step.text?{ru:step.text,en:step.text}:null}))});

const labels={
  field:['Поле агентов','The agents\' field'],
  lead:['Здесь агенты строят своё: объявляют виды, кладут мысли и связи, предлагают линзы и маршруты. Всё это их предложения в вашем журнале; в тикет они переходят только вашим действием.','Here agents build their own: they declare kinds, lay down thoughts and links, offer lenses and routes. All of it is their proposal in your journal; it becomes a ticket only by your act.'],
  kinds:['Виды, объявленные агентами','Kinds agents declared'],
  thoughts:['Мысли и связи','Thoughts and links'],
  lenses:['Линзы','Lenses'],
  routes:['Маршруты','Routes'],
  others:['Прочее в поле','Else in the field'],
  undeclared:['без объявления','undeclared'],
  shape:['форма','shape'],
  acts:['действий','acts'],
  toTicket:['В тикет','To a ticket'],
  show:['Показать','Show'],
  tryOn:['Примерить','Try on'],
  takeOff:['Снять','Take off'],
  walk:['Пройти','Walk'],
  empty:['Поле пока пусто: агенты объявляют виды записью kind-declaration','The field is empty yet: agents declare kinds with a kind-declaration act'],
  members:['звёзд','stars'],
  steps:['шагов','steps'],
  close:['Закрыть','Close'],
};

export function mountAgentField({shell,actionLayer,acts,actDesk=null,space=null,registry=null,locale=()=> 'ru'}){
  const ru=()=>resolveLocale(locale())==='ru';
  const t=key=>ru()?labels[key][0]:localeText(labels[key][1],locale());
  const doc=shell.ownerDocument;
  const el=(tag,cls='',value)=>{const node=doc.createElement(tag);if(cls)node.className=cls;if(value!==undefined)node.textContent=value;return node;};
  const button=(value,cls='text-button')=>{const node=el('button',cls,value);node.type='button';return node;};
  const nameOf=id=>{const item=space?.index?.get?.(id);return item?localeValue(item.title,locale()):id;};
  const who=author=>[author?.id,author?.model,author?.role].filter(Boolean).join(' · ');
  const dialogs=[];
  async function records(){const rows=[];let before=Number.MAX_SAFE_INTEGER;for(;;){const batch=await acts.recent({before,limit:500});if(!batch.length)break;rows.push(...batch.map(row=>row.record));before=batch.at(-1).seq;if(batch.length<500)break;}return rows;}
  const select=id=>registry?registry.invoke('tos.page.select',{item_id:id}).catch(()=>null):null;

  // An item becomes a ticket only by the person's act: the ticket keeps the field's act as its parent.
  function ticketFrom(item){
    const target=item.shape==='link'?item.from:item.targets[0]??item.members?.[0]??item.steps?.[0]?.target;if(!target||!actDesk)return;
    const record=space?.index?.get?.(target);
    const observation=item.shape==='link'?`${nameOf(item.from)} — ${item.verb} — ${nameOf(item.to)}${item.text?`\n${item.text}`:''}`:item.shape==='thought'?`${item.title}\n${item.text}`:`${item.title}${item.description?`\n${item.description}`:''}`;
    void actDesk.openTicket({type:record?.kind==='relation'?'link':'star',id:target,label:nameOf(target),item:record},{observation:`${observation}\n(${who(item.author)})`.slice(0,4000),parents:[item.id]});
  }

  async function open(){
    const dialog=el('dialog','ad-sheet af-sheet'),head=el('div','ad-head'),close=button('×','icon ad-close'),body=el('div','ad-body');
    close.setAttribute('aria-label',t('close'));close.addEventListener('click',()=>dialog.close());
    dialog.addEventListener('click',event=>{if(event.target===dialog)dialog.close();});dialog.addEventListener('close',()=>dialog.remove());
    head.append(el('h2','',t('field')),close);dialog.append(head,body);shell.append(dialog);dialogs.push(dialog);dialog.showModal?.();
    const field=fieldOf(await records());
    body.append(el('p','ad-muted',t('lead')));
    if(!field.kinds.length&&!field.others.length){body.append(el('p','ad-muted',t('empty')));return {dialog,field};}
    const section=(title,count)=>{const box=el('section','af-section');box.append(el('h3','ad-minor',`${title} · ${count}`));body.append(box);return box;};
    const kinds=section(t('kinds'),field.kinds.length);
    for(const kind of field.kinds){const row=el('div','af-kind');row.append(el('code','',kind.name),el('span','',kind.description),el('span','ad-muted',[kind.shape?`${t('shape')}: ${kind.shape}`:'',`${t('acts')}: ${kind.acts}`,who(kind.author)].filter(Boolean).join(' · ')));kinds.append(row);}
    for(const name of field.undeclared){const row=el('div','af-kind');row.append(el('code','',name),el('span','ad-muted',t('undeclared')));kinds.append(row);}
    const card=(item,title,lines)=>{const node=el('article','ad-group af-item');node.dataset.shape=item.shape??'';node.append(el('h3','',title));for(const line of lines.filter(Boolean))node.append(el('p','',line));node.append(el('p','ad-muted',`${item.kind} · ${who(item.author)}`));const row=el('div','ad-row');node.append(row);return {node,row};};
    const thoughts=field.items.filter(item=>item.shape==='thought'||item.shape==='link');
    if(thoughts.length){const box=section(t('thoughts'),thoughts.length);for(const item of thoughts){
      const {node,row}=item.shape==='link'?card(item,`${nameOf(item.from)} — ${item.verb} — ${nameOf(item.to)}`,[item.text]):card(item,item.title,[item.text,item.targets.map(nameOf).join(', ')]);
      const show=button(t('show'));show.addEventListener('click',()=>{dialog.close();void select(item.shape==='link'?item.from:item.targets[0]);});
      const ticket=button(t('toTicket'),'primary');ticket.addEventListener('click',()=>{dialog.close();ticketFrom(item);});row.append(show,ticket);box.append(node);}}
    const lenses=field.items.filter(item=>item.shape==='lens');
    if(lenses.length){const box=section(t('lenses'),lenses.length);for(const item of lenses){
      const {node,row}=card(item,item.title,[item.description,`${t('members')}: ${item.members.length}`,...item.groups.map(group=>`${group.title}: ${group.members.map(nameOf).join(', ')}`)]);
      const on=button(t('tryOn'),'primary');on.addEventListener('click',()=>{dialog.close();space?.actions?.setTrialLens?.(item);});
      const off=button(t('takeOff'));off.addEventListener('click',()=>space?.actions?.setTrialLens?.(null));
      const ticket=button(t('toTicket'));ticket.addEventListener('click',()=>{dialog.close();ticketFrom(item);});row.append(on,off,ticket);box.append(node);}}
    const routes=field.items.filter(item=>item.shape==='route');
    if(routes.length){
      space?.actions?.setAgentRoutes?.(routes.map(item=>routeForWalker(item,{nameOf})));
      const box=section(t('routes'),routes.length);for(const item of routes){
        const {node,row}=card(item,item.title,[item.question,`${t('steps')}: ${item.steps.length} · ${item.steps.map(step=>step.title||nameOf(step.target)).join(' → ')}`]);
        const walk=button(t('walk'),'primary');walk.addEventListener('click',()=>{dialog.close();space?.actions?.startRoute?.(`agent:${item.id}`);});
        const ticket=button(t('toTicket'));ticket.addEventListener('click',()=>{dialog.close();ticketFrom(item);});row.append(walk,ticket);box.append(node);}}
    if(field.others.length){const box=section(t('others'),field.others.length);for(const item of field.others.slice(0,60)){const row=el('p','af-other');row.append(el('code','',item.kind),el('span','',` ${item.intent} · ${who(item.author)}`));box.append(row);}}
    return {dialog,field};
  }
  const remove=actionLayer?.addActions?.([{id:'agent-field',text:()=>t('field'),for:['any'],run:()=>open()}]);
  return {open,destroy(){remove?.();space?.actions?.setTrialLens?.(null);for(const dialog of dialogs.splice(0)){if(dialog.open)dialog.close();dialog.remove();}}};
}
