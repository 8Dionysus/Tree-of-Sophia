// Talk at a place (B6): a thread opens from its own address — a star, a link or a passage — and a dispute keeps its
// form: a thesis, its grounds (exact places), an objection, an answer. A thesis is a note at the address; every other
// move is a reply that names what it answers. People and agents take part alike, each move an act of the person's
// journal under its author; nothing here decides who is right.
import './act-desk.css';
import {localeText,localeValue,resolveLocale} from './locale.mjs';

export const MOVES=Object.freeze(['thesis','grounds','objection','answer']);
/** What may answer what: grounds and objections answer a thesis; an answer answers an objection; an objection may meet grounds or an answer. */
export const REPLIES=Object.freeze({thesis:['grounds','objection'],grounds:['objection'],objection:['answer'],answer:['objection']});

/** The threads at an address from its records: each thesis with its moves as a tree, oldest first. */
export function threadsOf(records){
  const moves=records.filter(record=>MOVES.includes(record.body?.move)).sort((a,b)=>Date.parse(a.at)-Date.parse(b.at));
  const children=new Map();
  for(const record of moves)if(record.body.move!=='thesis')for(const parent of record.parents??[])children.set(parent,[...(children.get(parent)??[]),record]);
  const grow=record=>({record,move:record.body.move,replies:(children.get(record.id)??[]).filter(child=>REPLIES[record.body.move]?.includes(child.body.move)).map(grow)});
  return moves.filter(record=>record.body.move==='thesis').map(grow);
}

const labels={
  title:['Разговор на месте','Talk at the place'],
  lead:['Нить у этого места: тезис, его основания, возражение и ответ. Каждый ход — ваше действие или действие агента в журнале; кто прав, здесь не решается.','The thread at this place: a thesis, its grounds, an objection and an answer. Each move is an act of yours or of an agent in the journal; who is right is not decided here.'],
  none:['Здесь ещё не говорили','No talk here yet'],
  thesis:['Тезис','Thesis'],
  grounds:['Основания','Grounds'],
  objection:['Возражение','Objection'],
  answer:['Ответ','Answer'],
  newThesis:['Новый тезис','New thesis'],
  text:['Текст хода','The move\'s text'],
  places:['Точные места через пробел','Exact places, separated by spaces'],
  add:['Добавить','Add'],
  close:['Закрыть','Close'],
  needPlaces:['Основаниям нужны точные места','Grounds need exact places'],
};

export function mountTalk({shell,actionLayer,acts,space=null,locale=()=> 'ru'}){
  if(!acts)return {open:async()=>null,destroy(){}};
  const t=key=>resolveLocale(locale())==='ru'?labels[key][0]:localeText(labels[key][1],locale());
  const doc=shell.ownerDocument;
  const el=(tag,cls='',text)=>{const node=doc.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=text;return node;};
  const button=(text,cls='text-button')=>{const node=el('button',cls,text);node.type='button';return node;};
  const who=author=>author?.type==='human'?'':[author?.id,author?.model].filter(Boolean).join(' · ');
  const dialogs=[];

  const addressOf=target=>target?.id??(target?.passage?.reference?`passage:${target.passage.reference}`:null);

  async function recordsAt(address){
    const fold=await acts.query({view:'target',address,limit:500});
    const ids=fold.acts.filter(act=>(act.kind==='note'||act.kind==='reply')&&!act.withdrawn).map(act=>act.id);
    const records=await (acts.records?acts.records(ids):Promise.all(ids.map(id=>acts.record(id))));
    return records.filter(Boolean);
  }

  /** One move: a thesis is a note at the address; every other move replies to the act it answers. */
  async function say(address,move,text,{parent=null,grounds=[]}={}){
    const value=String(text??'').trim();if(!value||!MOVES.includes(move))return null;
    if(move==='grounds'&&!grounds.length)throw new Error(t('needPlaces'));
    return acts.act(move==='thesis'
      ?{kind:'note',targets:[address],grounds,intent:value.slice(0,200),body:{text:value,move}}
      :{kind:'reply',targets:[address],grounds,parents:[parent],intent:`${move}: ${value}`.slice(0,200),body:{text:value,move}});
  }

  async function open(target){
    const address=addressOf(target);if(!address)return null;
    const label=target.label??localeValue(space?.index?.get?.(address)?.title,locale())??address;
    const dialog=el('dialog','ad-sheet tk-sheet'),head=el('div','ad-head'),close=button('×','icon ad-close'),body=el('div','ad-body');
    close.setAttribute('aria-label',t('close'));close.addEventListener('click',()=>dialog.close());
    head.append(el('h2','',`${t('title')} · ${label}`),close);dialog.append(head,body);shell.append(dialog);dialogs.push(dialog);
    dialog.addEventListener('click',event=>{if(event.target===dialog)dialog.close();});dialog.addEventListener('close',()=>dialog.remove());
    const form=(move,parent)=>{
      const box=el('div','tk-form'),text=el('textarea','ad-input');text.rows=2;text.placeholder=t('text');text.setAttribute('aria-label',`${t(move)}: ${t('text')}`);
      const places=el('input','ad-input');places.placeholder=t('places');places.setAttribute('aria-label',t('places'));places.hidden=move!=='grounds';
      const add=button(`${t('add')}: ${t(move)}`,'primary');
      add.addEventListener('click',async()=>{
        try{await say(address,move,text.value,{parent,grounds:places.value.split(/\s+/).filter(Boolean)});await paint();}
        catch(error){actionLayer?.say?.(error.message);places.focus();}
      });
      box.append(text,places,add);return box;
    };
    const node=item=>{
      const box=el('article',`tk-move tk-${item.move}`);box.dataset.move=item.move;box.dataset.act=item.record.id;
      const by=who(item.record.author);
      box.append(el('p','tk-head',`${t(item.move)}${by?` · ${by}`:''}`),el('p','tk-text',item.record.body.text));
      if(item.record.grounds?.length)box.append(el('p','ad-muted',item.record.grounds.join(' · ')));
      const replies=el('div','tk-replies');for(const reply of item.replies)replies.append(node(reply));
      const row=el('div','ad-row');
      for(const move of REPLIES[item.move]??[]){const go=button(t(move));go.addEventListener('click',()=>{row.replaceWith(form(move,item.record.id));});row.append(go);}
      box.append(replies,row);return box;
    };
    async function paint(){
      const threads=threadsOf(await recordsAt(address));
      const out=[el('p','ad-muted',t('lead'))];
      if(!threads.length)out.push(el('p','',t('none')));
      for(const thread of threads)out.push(node(thread));
      out.push(el('h3','ad-minor',t('newThesis')),form('thesis',null));
      body.replaceChildren(...out);
    }
    await paint();dialog.showModal?.();return dialog;
  }
  const remove=actionLayer?.addActions?.([{id:'talk',text:()=>t('title'),for:['star','link','passage'],run:target=>open(target)}]);
  return {open,say,destroy(){remove?.();for(const dialog of dialogs.splice(0)){if(dialog.open)dialog.close();dialog.remove();}}};
}
