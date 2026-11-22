// Agents on the sky (C7, TOS-D-0082): where agents worked in the last 15 minutes, read from the journal's presence
// fold. One agent at a star is a small mark beside it; several are a ring whose size grows with their number; near
// enough, the ring names their roles. The legend says what the mark and the ring show; nothing here weighs a star.
// The person turns the agents' sky on and off; the choice stays in this browser.
import {localeText,resolveLocale} from './locale.mjs';

export const SKY_AGENTS_KEY='tos.public-research.sky-agents.v1';
export const SKY_AGENTS_WINDOW_MS=15*60*1000;

/** Presence rows as the sky reads them: address → {count, roles}, the roles most active first. */
export function presenceMap(presence){
  const out=new Map();
  for(const row of presence?.targets??[]){
    const roles=[...(row.roles??[])].sort((a,b)=>b.acts-a.acts).map(item=>item.role).filter(Boolean);
    out.set(row.target,{count:row.agents,roles});
  }
  return out;
}

const labels={
  title:['Агенты на небе','Agents on the sky'],
  legend:['Метка у звезды: здесь за 15 минут работал один агент. Кольцо: несколько агентов, его размер растёт с их числом. Вблизи кольцо называет их роли.','A mark beside a star: one agent worked here in the last 15 minutes. A ring: several agents; its size grows with their number. Near enough, the ring names their roles.'],
  on:['Агенты на небе показаны','Agents are shown on the sky'],
  off:['Агенты на небе скрыты','Agents are hidden from the sky'],
};

export function mountSkyAgents({actionLayer,acts,space,locale=()=> 'ru',storage=globalThis.localStorage,view=globalThis}){
  const slot=space?.actions?.agentsLegend?.();
  if(!acts||!space?.actions?.setAgentPresence)return {set:async()=>false,on:()=>false,destroy(){}};
  const t=key=>resolveLocale(locale())==='ru'?labels[key][0]:localeText(labels[key][1],locale());
  let on=false;try{on=storage?.getItem(SKY_AGENTS_KEY)==='on';}catch{}
  let timer=0,clock=0,disposed=false;

  async function paint(){
    if(disposed)return;
    if(!on){space.actions.setAgentPresence(null);if(slot){slot.hidden=true;slot.replaceChildren();}return;}
    let presence=null;
    try{presence=await acts.query({view:'presence',now_ms:Date.now(),window_ms:SKY_AGENTS_WINDOW_MS,limit:1000});}catch{}
    if(disposed||!on)return;
    space.actions.setAgentPresence(presenceMap(presence));
    if(slot){
      const doc=slot.ownerDocument,title=doc.createElement('span'),caption=doc.createElement('span');
      title.className='ps-legend-title';title.textContent=`${t('title')} · ${presence?.agents??0}`;
      caption.className='ps-highlight-caption';caption.textContent=t('legend');
      slot.replaceChildren(title,caption);slot.hidden=false;
    }
  }
  // A new act repaints the agents at most once a second; the window moves on every 15 seconds.
  const soon=()=>{if(timer||!on)return;timer=view.setTimeout(()=>{timer=0;void paint();},1000);};
  const stop=acts.subscribe?.(soon);
  const tick=()=>{view.clearInterval(clock);clock=on?view.setInterval(()=>void paint(),15000):0;};
  async function set(value){
    on=Boolean(value);try{storage?.setItem(SKY_AGENTS_KEY,on?'on':'off');}catch{}
    tick();await paint();actionLayer?.say?.(t(on?'on':'off'));return on;
  }
  const remove=actionLayer?.addActions?.([{id:'sky-agents',text:()=>t('title'),for:['any'],active:()=>on,run:()=>set(!on)}]);
  tick();void paint();
  return {set,on:()=>on,paint,destroy(){disposed=true;remove?.();stop?.();view.clearTimeout(timer);view.clearInterval(clock);space.actions.setAgentPresence(null);slot?.replaceChildren();}};
}
