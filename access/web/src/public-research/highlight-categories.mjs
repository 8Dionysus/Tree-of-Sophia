// Highlight categories of the research layer (A2, TOS-D-0082): one recorded property of the map at a time takes the
// channel of brightness and line weight (links) or size and brightness (stars), and the legend names what it shows.
// Nothing is weighed by the view itself: each category reads a property the stand or the person's journal recorded.
import {localeText,resolveLocale} from './locale.mjs';

/** The categories and what each reads: `edges` are core link IDs, `nodes` subject IDs. */
export const CATEGORIES=Object.freeze([
  {id:'verdict',channel:'links',read:({core})=>({edges:(core?.links??[]).filter(link=>['revise','reconstruction','rejected'].includes(link.assessment?.verdict)).map(link=>link.id)})},
  {id:'counter',channel:'links',read:({core})=>({edges:(core?.links??[]).filter(link=>link.assessment?.counter_reading).map(link=>link.id)})},
  {id:'attention',channel:'links',read:({core})=>({edges:(core?.links??[]).filter(link=>link.assessment?.attention==='high').map(link=>link.id)})},
  {id:'tickets',channel:'stars',read:({tickets})=>({nodes:[...(tickets??[])]})},
  {id:'agents',channel:'stars',read:({agents})=>({nodes:[...(agents??[])]})},
]);

/** The sets one category lights, from the stand's core and the person's journal. */
export function categorySets(id,sources){
  const category=CATEGORIES.find(item=>item.id===id);if(!category)return null;
  const read=category.read(sources);return {edges:read.edges??[],nodes:read.nodes??[],channel:category.channel};
}

const labels={
  title:['Подсветка','Highlight'],
  none:['Нет','None'],
  verdict:['Вердикт проверки','Review verdict'],
  counter:['Контрпрочтение','Counter-reading'],
  attention:['Внимание проверки','Review attention'],
  tickets:['Мои тикеты','My tickets'],
  agents:['Работа агентов','Agents at work'],
  'legend-verdict':['Яркость и вес линии: связи, которые проверка исправила, сочла реконструкцией или отвергла','Brightness and line weight: links review revised, found a reconstruction or rejected'],
  'legend-counter':['Яркость и вес линии: связи, у которых записано контрпрочтение','Brightness and line weight: links with a recorded counter-reading'],
  'legend-attention':['Яркость и вес линии: связи, которым проверка дала высокое внимание','Brightness and line weight: links review gave high attention'],
  'legend-tickets':['Размер и яркость: звёзды и связи, о которых у вас есть открытый тикет','Size and brightness: stars and links you have an open ticket on'],
  'legend-agents':['Размер и яркость: места, где агенты работали последние 15 минут','Size and brightness: places agents worked on in the last 15 minutes'],
};

export function mountHighlight({actionLayer,acts=null,space,locale=()=> 'ru'}){
  const slot=space?.actions?.highlightLegend?.();if(!slot||!space?.actions?.core?.())return {set:async()=>false,destroy(){}};
  const t=key=>resolveLocale(locale())==='ru'?labels[key][0]:localeText(labels[key][1],locale());
  const doc=slot.ownerDocument;
  const el=(tag,cls='',value)=>{const node=doc.createElement(tag);if(cls)node.className=cls;if(value!==undefined)node.textContent=value;return node;};
  let current=null;
  async function sources(){
    const out={core:space.actions.core(),tickets:new Set(),agents:new Set()};
    if(!acts)return out;
    try{const board=await acts.query({view:'board',limit:1000});for(const ticket of board.tickets)if(['open','with-agent','returned'].includes(ticket.status))for(const target of ticket.targets??[])out.tickets.add(target);}catch{}
    try{const presence=await acts.query({view:'presence',now_ms:Date.now(),window_ms:15*60*1000,limit:500});for(const row of presence.targets??[])out.agents.add(row.target);}catch{}
    return out;
  }
  async function set(id){
    current=id&&CATEGORIES.some(item=>item.id===id)?id:null;
    if(!current){space.actions.setHighlight(null);paint();return false;}
    const found=categorySets(current,await sources());
    // A ticket names a link or a star by its address: both channels take it.
    space.actions.setHighlight({edges:[...found.edges,...(found.channel==='stars'?found.nodes:[])],nodes:found.nodes});
    paint();return true;
  }
  function paint(){
    const research=actionLayer?.layer?.()==='research';slot.hidden=!research;
    slot.replaceChildren(el('span','ps-legend-title',t('title')));
    const items=el('div','ps-highlight-items');
    for(const id of [null,...CATEGORIES.map(item=>item.id)]){
      const chip=el('button','ps-highlight-chip',id?t(id):t('none'));chip.type='button';chip.dataset.category=id??'none';
      chip.setAttribute('aria-pressed',String(current===id));chip.addEventListener('click',()=>void set(id));items.append(chip);
    }
    if(current)items.append(el('span','ps-highlight-caption',t(`legend-${current}`)));
    slot.append(items);
  }
  // Leaving the research layer puts the highlight out; a new act repaints a category read from the journal.
  const onLayer=()=>{if(actionLayer?.layer?.()!=='research'&&current){current=null;space.actions.setHighlight(null);}paint();};
  space.root?.addEventListener?.('public-research-layer',onLayer);
  const stop=acts?.subscribe?.(()=>{if(current==='tickets'||current==='agents')void set(current);});
  const remove=actionLayer?.addActions?.(CATEGORIES.map(item=>({id:`highlight-${item.id}`,text:()=>`${t('title')}: ${t(item.id)}`,for:['any'],when:()=>actionLayer?.layer?.()==='research',active:()=>current===item.id,run:()=>set(current===item.id?null:item.id)})));
  paint();
  return {set,current:()=>current,destroy(){remove?.();stop?.();space.root?.removeEventListener?.('public-research-layer',onLayer);space.actions.setHighlight(null);slot.replaceChildren();}};
}
