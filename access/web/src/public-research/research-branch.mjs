// Research branches (B5): a branch ties a question to what the person gathers under it — tickets (as the branch act's
// parents), thoughts of "My research", the layout of the view (a link that opens it as it was) and the session the
// work goes on in. A branch is the person's act (`branch`, act-kinds.v1.json); changing it appends a new branch act
// that replaces the old one, so the branch keeps its history.
import './act-desk.css';
import {localeText,resolveLocale} from './locale.mjs';

export const MAX_BRANCH_TICKETS=32;

/** The live branches among records: the newest act for each `branch:` address, newest first. */
export function branchesOf(records){
  const latest=new Map();
  for(const record of records){
    if(record.kind!=='branch')continue;
    const address=record.targets?.[0];if(!address)continue;
    const known=latest.get(address);
    if(!known||Date.parse(record.at)>Date.parse(known.at))latest.set(address,record);
  }
  return [...latest.values()].sort((a,b)=>Date.parse(b.at)-Date.parse(a.at)).map(record=>({address:record.targets[0],id:record.id,at:record.at,
    question:record.body.question,tickets:record.body.tickets??[],thoughts:record.body.thoughts??[],layout:record.body.layout??null,session:record.body.session??null}));
}

/** A branch's address from its question: `branch:` and a short slug, kept unique by a suffix. */
export function branchAddress(question,taken=new Set()){
  const base=String(question??'').toLowerCase().normalize('NFKD').replace(/[^\p{L}\p{N}]+/gu,'-').replace(/^-+|-+$/g,'').slice(0,48)||'branch';
  let address=`branch:${base}`,n=2;while(taken.has(address))address=`branch:${base}-${n++}`;
  return address;
}

const labels={
  title:['Ветки исследования','Research branches'],
  lead:['Ветка связывает вопрос с тикетами, мыслями, раскладкой вида и сессией. Изменение ветки — новое действие, прежнее остаётся в её истории.','A branch ties a question to tickets, thoughts, the view\'s layout and a session. Changing a branch is a new act; the earlier one stays in its history.'],
  none:['Веток пока нет','No branches yet'],
  question:['Вопрос ветки','The branch\'s question'],
  tickets:['Тикеты','Tickets'],
  thoughts:['Мысли','Thoughts'],
  layout:['Раскладка вида','The view\'s layout'],
  session:['Сессия','Session'],
  keepLayout:['Сохранить нынешнюю раскладку','Keep the present layout'],
  keepSession:['Связать с нынешней сессией','Tie to the present session'],
  openLayout:['Открыть раскладку','Open the layout'],
  create:['Открыть ветку','Open the branch'],
  update:['Сохранить ветку','Save the branch'],
  newBranch:['Новая ветка','New branch'],
  close:['Закрыть','Close'],
  saved:['Ветка сохранена','The branch is saved'],
};

export function mountResearchBranches({shell,actionLayer,acts,api=null,space=null,getSession=()=>null,locale=()=> 'ru',view=globalThis}){
  if(!acts)return {open:async()=>null,save:async()=>null,destroy(){}};
  const t=key=>resolveLocale(locale())==='ru'?labels[key][0]:localeText(labels[key][1],locale());
  const doc=shell.ownerDocument;
  const el=(tag,cls='',text)=>{const node=doc.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=text;return node;};
  const button=(text,cls='text-button')=>{const node=el('button',cls,text);node.type='button';return node;};
  const dialogs=[];
  const nameOf=address=>{const item=space?.index?.get?.(address);const title=item?.title;return title?(typeof title==='string'?title:title[resolveLocale(locale())==='ru'?'ru':'en']??title.en??address):address;};

  async function records(limit=3000){const rows=[];let before=Number.MAX_SAFE_INTEGER;while(rows.length<limit){const batch=await acts.recent({before,limit:500});if(!batch.length)break;rows.push(...batch.map(row=>row.record));before=batch.at(-1).seq;if(batch.length<500)break;}return rows;}

  /** Save a branch: a new branch act at its address, replacing the one before it. */
  async function save({address=null,question,tickets=[],thoughts=[],layout=null,session=null,replaces=null},taken=new Set()){
    const text=String(question??'').replace(/\s+/g,' ').trim();if(!text)return null;
    const at=address??branchAddress(text,taken);
    const parents=tickets.slice(0,MAX_BRANCH_TICKETS);
    const stored=await acts.act({kind:'branch',targets:[at],parents,intent:text.slice(0,200),...(replaces?{replaces}:{}),
      body:{question:text,tickets:parents,thoughts,...(layout?{layout}:{}),...(session?{session}:{})}});
    actionLayer?.say?.(t('saved'));return stored;
  }

  async function open(){
    const dialog=el('dialog','ad-sheet rb-sheet'),head=el('div','ad-head'),close=button('×','icon ad-close'),body=el('div','ad-body');
    close.setAttribute('aria-label',t('close'));close.addEventListener('click',()=>dialog.close());
    head.append(el('h2','',t('title')),close);dialog.append(head,body);shell.append(dialog);dialogs.push(dialog);
    dialog.addEventListener('click',event=>{if(event.target===dialog)dialog.close();});dialog.addEventListener('close',()=>dialog.remove());
    async function paint(){
      const rows=await records(),branches=branchesOf(rows);
      const board=await acts.query({view:'board',limit:200}).catch(()=>({tickets:[]}));
      const thoughtIds=[...new Set(rows.filter(record=>record.kind==='workspace-event').flatMap(record=>record.targets??[]).filter(id=>!id.startsWith('workspace:')))].slice(0,40);
      const out=[el('p','ad-muted',t('lead'))];
      if(!branches.length)out.push(el('p','',t('none')));
      const editor=(branch=null)=>{
        const box=el('div','rb-editor');
        const question=el('input','ad-input');question.value=branch?.question??api?.getQuestion?.()??'';question.setAttribute('aria-label',t('question'));question.placeholder=t('question');
        const check=(group,value,label,on)=>{const row=el('label','ad-check-row'),input=el('input');input.type='checkbox';input.value=value;input.checked=on;input.dataset.group=group;row.append(input,el('span','',label));return row;};
        const tickets=el('fieldset','rb-group');tickets.append(el('legend','',t('tickets')));
        for(const ticket of board.tickets.slice(0,60))tickets.append(check('ticket',ticket.id,`${ticket.category} · ${nameOf(ticket.targets?.[0]??ticket.id)} · ${ticket.status}`,branch?.tickets.includes(ticket.id)??false));
        const thoughts=el('fieldset','rb-group');thoughts.append(el('legend','',t('thoughts')));
        for(const id of thoughtIds)thoughts.append(check('thought',id,id,branch?.thoughts.includes(id)??false));
        const layout=check('layout','layout',t('keepLayout'),!branch||Boolean(branch.layout));
        const session=getSession?.();const sessionRow=session?check('session',session,`${t('keepSession')} · ${session}`,!branch||branch.session===session):null;
        const go=button(branch?t('update'):t('create'),'primary');
        go.addEventListener('click',async()=>{
          const picked=group=>[...box.querySelectorAll(`input[data-group="${group}"]`)].filter(input=>input.checked).map(input=>input.value);
          const keepLayout=layout.querySelector('input').checked;
          await save({address:branch?.address??null,replaces:branch?.id??null,question:question.value,tickets:picked('ticket'),thoughts:picked('thought'),
            layout:keepLayout?(actionLayer?.linkTo?.(actionLayer?.currentTarget?.())??view.location?.href??null):null,
            session:picked('session')[0]??null},new Set(branches.map(item=>item.address)));
          await paint();
        });
        box.append(question,tickets,thoughts,layout,...(sessionRow?[sessionRow]:[]),go);return box;
      };
      for(const branch of branches){
        const card=el('article','ad-group rb-item');card.dataset.branch=branch.address;
        card.append(el('h3','',branch.question),el('p','ad-muted',`${t('tickets')}: ${branch.tickets.length} · ${t('thoughts')}: ${branch.thoughts.length}${branch.session?` · ${t('session')}: ${branch.session}`:''}`));
        const row=el('div','ad-row');
        if(branch.layout){const go=button(t('openLayout'));go.addEventListener('click',()=>{dialog.close();view.location.assign?.(branch.layout);});row.append(go);}
        const edit=button(t('update'));edit.addEventListener('click',()=>{row.replaceWith(editor(branch));});row.append(edit);
        card.append(row);out.push(card);
      }
      out.push(el('h3','ad-minor',t('newBranch')),editor());
      body.replaceChildren(...out);
    }
    await paint();dialog.showModal?.();return dialog;
  }
  const remove=actionLayer?.addActions?.([{id:'branches',text:()=>t('title'),for:['any'],run:()=>open()}]);
  return {open,save,destroy(){remove?.();for(const dialog of dialogs.splice(0)){if(dialog.open)dialog.close();dialog.remove();}}};
}
