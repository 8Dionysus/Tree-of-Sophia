// What review withheld (A5, TOS-D-0082): the nodes, links, routes and sources the stand took off its strict map and
// keeps whole (`bonsai/<stand>/held.json`) show only in the research layer and only on request, each with the reason
// review gave. They stay the stand's record for later growth: the stand binds none of them, and nothing here
// changes them.
import './act-desk.css';
import {localeText,localeValue,resolveLocale} from './locale.mjs';
import {technicalSection} from './technical.mjs';

const HELD={zarathustra:()=>import('../../../../bonsai/zarathustra/held.json').then(module=>module.default??module)};
const pair=(ru,en)=>ru||en?{ru:ru??'',en:en??''}:null;
const bare=id=>String(id??'').replace(/^(material:)?(demo:|core:)?/,'');

/** The held record in one shape: nodes, links, routes and sources, each with its reason. */
export function heldOf(held){
  const nodes=(held?.nodes??[]).map(item=>({id:item.row?.[0],kind:item.row?.[1],title:pair(item.row?.[3],item.row?.[4]),reason:item.reason??'',body:item.content?.body??null,question:item.content?.question??null})).filter(item=>item.id);
  const edges=(held?.edges??[]).map(item=>({from:item.row?.[0],to:item.row?.[1],relation:item.row?.[2],label:pair(item.row?.[3],item.row?.[4]),text:pair(item.row?.[5],item.row?.[6]),verdict:item.verdict??'',reason:item.reason??'',places:item.places??[]})).filter(item=>item.from&&item.to);
  const simple=list=>(list??[]).filter(item=>item?.id).map(item=>({id:item.id,reason:item.reason??''}));
  return {nodes,edges,routes:simple(held?.routes),sources:simple(held?.sources)};
}

/** What review withheld at one subject: the held node itself and the held links that touch it. */
export function heldAt(record,id){
  const key=bare(id);if(!key)return {nodes:[],edges:[]};
  return {nodes:record.nodes.filter(item=>bare(item.id)===key),edges:record.edges.filter(item=>bare(item.from)===key||bare(item.to)===key)};
}

const labels={
  held:['Удержанное проверкой','What review withheld'],
  lead:['Что проверка сняла со строгой карты, целиком и с причиной. В чтении стенд этого не показывает и ни с чем не связывает.','What review took off the strict map, whole and with its reason. The stand does not show it in reading and binds none of it.'],
  nodes:['Узлы','Nodes'],
  edges:['Связи','Links'],
  routes:['Маршруты','Routes'],
  sources:['Источники','Sources'],
  reason:['Причина','Reason'],
  verdict:['Вердикт','Verdict'],
  places:['мест','places'],
  nothing:['Здесь проверка ничего не удержала','Review withheld nothing here'],
  onlyResearch:['Удержанное видно только в слое «Исследование»','What review withheld shows only in the research layer'],
  close:['Закрыть','Close'],
};

export function mountHeld({shell,actionLayer,space=null,locale=()=> 'ru',profile='zarathustra',load=HELD[profile]??null}){
  if(!load)return {open:async()=>{},destroy(){}};
  const t=key=>resolveLocale(locale())==='ru'?labels[key][0]:localeText(labels[key][1],locale());
  const doc=shell.ownerDocument;
  const el=(tag,cls='',value)=>{const node=doc.createElement(tag);if(cls)node.className=cls;if(value!==undefined)node.textContent=value;return node;};
  const button=(value,cls='text-button')=>{const node=el('button',cls,value);node.type='button';return node;};
  const form=value=>value==null?'':typeof value==='string'?value:localeValue(value,locale());
  const nameOf=id=>{const item=space?.index?.get?.(bare(id))??space?.index?.get?.(id);return item?localeValue(item.title,locale()):bare(id);};
  let record=null;const read=async()=>record??=heldOf(await load());
  const dialogs=[];
  const inResearch=()=>actionLayer?.layer?.()==='research';

  async function open(target=null){
    if(!inResearch()){actionLayer?.say?.(t('onlyResearch'));return null;}
    const all=await read(),at=target?.id?heldAt(all,target.id):null;
    const dialog=el('dialog','ad-sheet hv-sheet'),head=el('div','ad-head'),close=button('×','icon ad-close'),body=el('div','ad-body');
    close.setAttribute('aria-label',t('close'));close.addEventListener('click',()=>dialog.close());
    dialog.addEventListener('click',event=>{if(event.target===dialog)dialog.close();});dialog.addEventListener('close',()=>dialog.remove());
    head.append(el('h2','',at?`${t('held')} · ${target.label??nameOf(target.id)}`:t('held')),close);dialog.append(head,body);shell.append(dialog);dialogs.push(dialog);
    body.append(el('p','ad-muted',t('lead')));
    const groups=at?{nodes:at.nodes,edges:at.edges,routes:[],sources:[]}:all;
    if(at&&!at.nodes.length&&!at.edges.length)body.append(el('p','',t('nothing')));
    const entry=(title,reason,details=[])=>{const box=el('article','ad-group hv-item');box.append(el('h3','',title),el('p','hv-reason',`${t('reason')}: ${reason||'—'}`));const apparatus=details.filter(Boolean);if(apparatus.length)box.append(technicalSection(doc,locale,apparatus));return box;};
    for(const [key,items] of Object.entries(groups)){
      if(!items.length)continue;
      const section=el('section','hv-section');section.dataset.group=key;section.append(el('h3','ad-minor',`${t(key)} · ${items.length}`));
      for(const item of items){
        if(key==='nodes')section.append(entry(form(item.title)||item.id,item.reason,[item.body?el('p','',form(item.body)):null,item.question?el('p','',form(item.question)):null,el('p','',`${item.kind??''} · ${item.id}`)]));
        else if(key==='edges')section.append(entry(`${nameOf(item.from)} — ${form(item.label)||item.relation} — ${nameOf(item.to)}`,item.reason,[item.text?el('p','',form(item.text)):null,el('p','',`${t('verdict')}: ${item.verdict} · ${item.relation} · ${item.places.length} ${t('places')}`)]));
        else section.append(entry(item.id,item.reason));
      }
      body.append(section);
    }
    dialog.showModal?.();return dialog;
  }
  const remove=actionLayer?.addActions?.([{id:'held',text:()=>t('held'),for:['any'],when:()=>inResearch(),run:target=>open(target?.type==='star'||target?.type==='link'?target:null)}]);
  return {open,destroy(){remove?.();for(const dialog of dialogs.splice(0)){if(dialog.open)dialog.close();dialog.remove();}}};
}
