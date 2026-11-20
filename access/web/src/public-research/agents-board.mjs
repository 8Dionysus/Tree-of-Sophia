// The workstation (G3): the board "Agents" shows who works in the person's act journal now: each agent with its
// model, role and squad, how many acts it made in the window and where it last worked; the places agents work on;
// the places held by a lease, a clash of two holders marked; and what waits for the person's review. The board opens
// in the stand, or in a window of its own that the person moves to a second screen (placed there at once where the
// browser lets the page see its screens). Everything is read from the journal; the board changes nothing.
import './act-desk.css';
import './agents-board.css';
import {localeText,localeValue,resolveLocale} from './locale.mjs';

export const BOARD_WINDOW_MS=15*60*1000;

/** Agents in the newest records: each with its model, role, squad, count of acts, last time and last place. */
export function agentsOf(records,{now=Date.now(),window=BOARD_WINDOW_MS}={}){
  const agents=new Map(),since=now-window;
  for(const record of records){
    if(record.author?.type!=='agent')continue;
    const at=Date.parse(record.at);if(!(at>=since))continue;
    const id=record.author.id,row=agents.get(id)??{id,model:record.author.model??null,role:record.author.role??null,squad:record.author.squad??null,acts:0,last:0,target:null,kinds:{}};
    row.acts++;row.kinds[record.kind]=(row.kinds[record.kind]??0)+1;
    if(at>row.last){row.last=at;row.target=record.targets?.[0]??null;row.role=record.author.role??row.role;row.squad=record.author.squad??row.squad;}
    agents.set(id,row);
  }
  return [...agents.values()].sort((a,b)=>b.last-a.last||a.id.localeCompare(b.id));
}

const labels={
  board:['Доска «Агенты»','The board “Agents”'],
  lead:['Кто работает в вашем журнале сейчас: агенты за последние 15 минут, места их работы, удержанные места и то, что ждёт рассмотрения.','Who works in your journal now: agents in the last 15 minutes, the places they work on, the places held and what waits for review.'],
  agents:['Агенты','Agents'],
  places:['Места работы','Places of work'],
  leases:['Удержанные места','Places held'],
  clash:['двое держат одно место','two hold one place'],
  waiting:['Ждут рассмотрения','Waiting for review'],
  groups:['групп','groups'],
  proposals:['предложений','proposals'],
  acts:['действий','acts'],
  none:['Агентов в журнале сейчас нет','No agents in the journal now'],
  noPlaces:['Пусто','Empty'],
  detach:['В отдельное окно','In a window of its own'],
  close:['Закрыть','Close'],
  ago:['{0} с назад','{0} s ago'],
  agoMin:['{0} мин назад','{0} min ago'],
};

export function mountAgentsBoard({shell,actionLayer,acts,space=null,locale=()=> 'ru',view=globalThis}){
  const ru=()=>resolveLocale(locale())==='ru';
  const t=key=>ru()?labels[key][0]:localeText(labels[key][1],locale());
  const nameOf=address=>{const item=space?.index?.get?.(address);return item?localeValue(item.title,locale()):address;};
  const live=new Set();

  async function records(limit=1000){const rows=[];let before=Number.MAX_SAFE_INTEGER;while(rows.length<limit){const batch=await acts.recent({before,limit:Math.min(500,limit-rows.length)});if(!batch.length)break;rows.push(...batch.map(row=>row.record));before=batch.at(-1).seq;if(batch.length<500)break;}return rows;}

  /** Draw the board into `root`, a node of any document (the stand's or a window of its own). */
  async function render(root,{now=Date.now()}={}){
    const doc=root.ownerDocument,el=(tag,cls='',text)=>{const node=doc.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=text;return node;};
    const [rows,presence,leases,review]=await Promise.all([records(),acts.query({view:'presence',now_ms:now,window_ms:BOARD_WINDOW_MS,limit:50}),acts.query({view:'leases',now_ms:now,limit:50}),acts.query({view:'review',limit:50})]);
    const agents=agentsOf(rows,{now});
    const ago=at=>{const s=Math.max(0,Math.round((now-at)/1000));return s<120?t('ago').replace('{0}',s):t('agoMin').replace('{0}',Math.round(s/60));};
    const section=(title,count)=>{const box=el('section','ab-section');box.append(el('h3','ad-minor',`${title}${count!==undefined?` · ${count}`:''}`));return box;};
    const out=[el('p','ad-muted',t('lead'))];
    const people=section(t('agents'),agents.length);
    if(!agents.length)people.append(el('p','ad-muted',t('none')));
    else{const table=el('div','ab-agents');for(const agent of agents.slice(0,200)){const row=el('div','ab-agent');row.dataset.agent=agent.id;
      row.append(el('strong','',agent.id),el('span','ab-meta',[agent.model,agent.role,agent.squad].filter(Boolean).join(' · ')),el('span','ab-count',`${t('acts')}: ${agent.acts}`),el('span','ab-where',agent.target?nameOf(agent.target):''),el('span','ab-when',ago(agent.last)));table.append(row);}
      people.append(table);}
    out.push(people);
    const places=section(t('places'),presence.targets_count);
    if(!presence.targets?.length)places.append(el('p','ad-muted',t('noPlaces')));
    for(const row of presence.targets??[]){const line=el('p','ab-line');line.append(el('span','',nameOf(row.target)),el('span','ab-count',`${row.agents} · ${row.roles.map(role=>`${role.role??'—'} ${role.acts}`).join(', ')}`));places.append(line);}
    out.push(places);
    const held=section(t('leases'),leases.leases.length);
    if(!leases.leases.length)held.append(el('p','ad-muted',t('noPlaces')));
    for(const row of leases.leases){const line=el('p','ab-line');if(row.collision)line.dataset.clash='true';line.append(el('span','',nameOf(row.target)),el('span','ab-count',`${row.holders.join(', ')}${row.collision?` · ${t('clash')}`:''}`));held.append(line);}
    out.push(held);
    const open=review.groups.reduce((sum,group)=>sum+group.open_count,0);
    const waiting=section(t('waiting'));waiting.append(el('p','ab-line',`${t('groups')}: ${review.groups.length} · ${t('proposals')}: ${open}`));
    out.push(waiting);
    root.replaceChildren(...out);
    return {agents,presence,leases,review};
  }

  // The board follows the journal: a new act repaints it, at most twice a second; the clock repaints it every 10 s.
  function follow(root,win=view){
    let timer=0,stopped=false;
    const paint=()=>{if(stopped||!root.isConnected)return;void render(root).catch(()=>{});};
    const soon=()=>{if(timer)return;timer=win.setTimeout(()=>{timer=0;paint();},500);};
    const stop=acts.subscribe?.(soon)??(()=>{});const clock=win.setInterval(paint,10000);
    const entry={stop(){stopped=true;stop();win.clearTimeout(timer);win.clearInterval(clock);live.delete(entry);}};live.add(entry);
    paint();return entry;
  }

  const dialogs=[];
  function open(){
    const doc=shell.ownerDocument,el=(tag,cls='',text)=>{const node=doc.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=text;return node;};
    const dialog=el('dialog','ad-sheet ab-sheet'),head=el('div','ad-head'),tools=el('div','ad-row'),close=el('button','icon ad-close','×'),body=el('div','ad-body');
    close.type='button';close.setAttribute('aria-label',t('close'));close.addEventListener('click',()=>dialog.close());
    const detach=el('button','text-button',t('detach'));detach.type='button';detach.addEventListener('click',()=>{dialog.close();void openWindow();});
    tools.append(detach,close);head.append(el('h2','',t('board')),tools);dialog.append(head,body);shell.append(dialog);dialogs.push(dialog);
    const following=follow(body);
    dialog.addEventListener('click',event=>{if(event.target===dialog)dialog.close();});dialog.addEventListener('close',()=>{following.stop();dialog.remove();});
    dialog.showModal?.();return dialog;
  }

  /** A window of its own; where the page may see the screens, it opens on another screen than the stand's. */
  async function openWindow(){
    let features='popup,width=560,height=820';
    try{
      if(view.getScreenDetails){const details=await view.getScreenDetails();const other=details.screens.find(screen=>screen!==details.currentScreen);
        if(other)features=`popup,left=${other.availLeft+40},top=${other.availTop+40},width=${Math.min(720,other.availWidth-80)},height=${other.availHeight-80}`;}
    }catch{}
    const win=view.open('','tos-agents-board',features);if(!win)return null;
    const doc=win.document;doc.title=t('board');doc.documentElement.lang=resolveLocale(locale());
    // The window shares the stand's styles: its stylesheets are named again in the new document.
    for(const node of shell.ownerDocument.querySelectorAll('link[rel="stylesheet"],style'))doc.head.append(node.cloneNode(true));
    doc.body.className='ab-window';doc.body.replaceChildren();
    const heading=doc.createElement('h2');heading.textContent=t('board');const root=doc.createElement('div');root.className='ad-body';
    doc.body.append(heading,root);
    const following=follow(root,win);win.addEventListener('pagehide',()=>following.stop(),{once:true});
    return win;
  }

  const remove=actionLayer?.addActions?.([
    {id:'agents-board',text:()=>t('board'),for:['any'],run:()=>open()},
    {id:'agents-board-window',text:()=>`${t('board')} · ${t('detach')}`,for:['any'],run:()=>openWindow()},
  ]);
  return {open,openWindow,render,destroy(){remove?.();for(const entry of [...live])entry.stop();for(const dialog of dialogs.splice(0)){if(dialog.open)dialog.close();dialog.remove();}}};
}
