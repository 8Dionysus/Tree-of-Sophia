// The return gateway (D3): what the person handed over (a transfer act: a quote, a packet or a task for an agent
// outside the page) and what came back. Each transfer shows where it left from, in what form, to whom and its state:
// sent, or returned with each return. A result pasted back is a return act that names the transfer as its parent and
// stands at the same place, so it is found where the work left from. Nothing here judges the result.
import './act-desk.css';
import {localeText,localeValue,resolveLocale} from './locale.mjs';

/** Transfers with their returns, newest first. */
export function transfersOf(records){
  const transfers=new Map(),returns=[];
  for(const record of records){
    if(record.kind==='transfer')transfers.set(record.id,{id:record.id,targets:record.targets??[],format:record.body?.format??null,recipient:record.body?.recipient??null,at:record.at,intent:record.intent,returns:[]});
    else if(record.kind==='return')returns.push(record);
  }
  for(const record of returns)for(const parent of record.parents??[]){const transfer=transfers.get(parent);if(transfer)transfer.returns.push({id:record.id,at:record.at,text:record.body?.text??'',author:record.author});}
  return [...transfers.values()].map(item=>({...item,state:item.returns.length?'returned':'sent',returns:item.returns.sort((a,b)=>Date.parse(a.at)-Date.parse(b.at))}))
    .sort((a,b)=>Date.parse(b.at)-Date.parse(a.at));
}

const labels={
  title:['Передачи и возвраты','Hand-overs and returns'],
  lead:['Что вы передали наружу и что вернулось. Вернувшийся результат встаёт в то место, откуда ушла передача.','What you handed outside and what came back. A returned result stands at the place the hand-over left from.'],
  none:['Передач пока нет: их начинает действие «Передать…» у звезды, связи или места','No hand-overs yet: “Hand over…” at a star, a link or a place starts one'],
  sent:['отправлено, ждёт возврата','sent, waiting for a return'],
  returned:['вернулось: {0}','returned: {0}'],
  form:['В каком виде','In what form'],
  to:['Кому','To whom'],
  open:['Открыть место','Open the place'],
  paste:['Вставьте результат: ответ агента или человека','Paste the result: the answer of an agent or a person'],
  bring:['Вернуть результат','Bring the result back'],
  close:['Закрыть','Close'],
  back:['Результат вернулся к месту','The result is back at its place'],
};

export function mountReturnGateway({shell,actionLayer,acts,space=null,experience=null,locale=()=> 'ru'}){
  if(!acts)return {open:async()=>null,destroy(){}};
  const t=key=>resolveLocale(locale())==='ru'?labels[key][0]:localeText(labels[key][1],locale());
  const doc=shell.ownerDocument;
  const el=(tag,cls='',text)=>{const node=doc.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=text;return node;};
  const button=(text,cls='text-button')=>{const node=el('button',cls,text);node.type='button';return node;};
  const nameOf=address=>{const item=space?.index?.get?.(address);return item?localeValue(item.title,locale()):address;};
  const when=at=>{const date=new Date(at);return Number.isFinite(date.getTime())?date.toLocaleString(resolveLocale(locale())):'';};
  const dialogs=[];

  async function records(limit=2000){const rows=[];let before=Number.MAX_SAFE_INTEGER;while(rows.length<limit){const batch=await acts.recent({before,limit:500});if(!batch.length)break;rows.push(...batch.map(row=>row.record));before=batch.at(-1).seq;if(batch.length<500)break;}return rows;}

  /** Bring a result back: a return act at the transfer's place, with the transfer as its parent. */
  async function bringBack(transfer,text){
    const value=String(text??'').trim();if(!value)return null;
    const stored=await acts.act({kind:'return',targets:transfer.targets.length?transfer.targets:[`act:${transfer.id}`],parents:[transfer.id],
      intent:value.split('\n')[0].slice(0,200),body:{text:value,format:'text'}});
    actionLayer?.say?.(t('back'));return stored;
  }

  async function open(){
    const dialog=el('dialog','ad-sheet rg-sheet'),head=el('div','ad-head'),close=button('×','icon ad-close'),body=el('div','ad-body');
    close.setAttribute('aria-label',t('close'));close.addEventListener('click',()=>dialog.close());
    head.append(el('h2','',t('title')),close);dialog.append(head,body);shell.append(dialog);dialogs.push(dialog);
    dialog.addEventListener('click',event=>{if(event.target===dialog)dialog.close();});dialog.addEventListener('close',()=>dialog.remove());
    const paint=async()=>{
      const list=transfersOf(await records());
      const out=[el('p','ad-muted',t('lead'))];
      if(!list.length)out.push(el('p','ad-muted',t('none')));
      for(const item of list.slice(0,100)){
        const card=el('article','ad-group rg-item');card.dataset.state=item.state;card.dataset.transfer=item.id;
        const place=item.targets[0]??'';
        card.append(el('h3','',place?nameOf(place):item.intent),
          el('p','ad-muted',`${t('form')}: ${item.format??'—'} · ${t('to')}: ${item.recipient??'—'} · ${when(item.at)}`),
          el('p','rg-state',item.state==='sent'?t('sent'):t('returned').replace('{0}',String(item.returns.length))));
        for(const back of item.returns){const quote=el('blockquote','rg-return',back.text);card.append(quote);}
        const row=el('div','ad-row');
        if(place&&experience?.open&&space?.index?.get?.(place)){const go=button(t('open'));go.addEventListener('click',()=>{dialog.close();experience.open(place);});row.append(go);}
        const field=el('textarea','ad-input rg-paste');field.rows=3;field.placeholder=t('paste');field.setAttribute('aria-label',t('paste'));
        const bring=button(t('bring'),'primary');bring.addEventListener('click',async()=>{if(await bringBack(item,field.value))await paint();});
        row.append(bring);card.append(field,row);out.push(card);
      }
      body.replaceChildren(...out);
    };
    await paint();dialog.showModal?.();return dialog;
  }
  const remove=actionLayer?.addActions?.([{id:'transfers',text:()=>t('title'),for:['any'],run:()=>open()}]);
  return {open,bringBack,destroy(){remove?.();for(const dialog of dialogs.splice(0)){if(dialog.open)dialog.close();dialog.remove();}}};
}
