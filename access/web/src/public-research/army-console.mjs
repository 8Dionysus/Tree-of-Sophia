// The army's console (K8, K1, K3): the person's view of hundreds of agents at once, read from the journal's folds.
// Where the journal runs (this browser or the person's bridge); the flow of acts; each squad with its role, its
// agents at work, what it spent against its budget, the share of its proposals the person accepted and its stalled
// agents; and the person's controls: a squad declared with its role, area, budget and stop rule, the agents' tier,
// and one pause that stops every agent act at the bridge. Nothing here decides for the person.
import roles from '../../../contracts/agent-roles.v1.json' with {type:'json'};

/** The squads fold with what the console shows: accepted share of decided proposals and spend against the budget. */
export function armyRows(squads){
  return (squads?.squads??[]).map(row=>{
    const decided=row.proposals.accepted+row.proposals.rejected;
    return {...row,
      accepted_share:decided?row.proposals.accepted/decided:null,
      spend:row.budget_acts?row.acts/row.budget_acts:null};
  });
}

/** Acts a minute in the window, from the squads' recent acts. */
export const flowOf=(squads,windowMs)=>Math.round((squads?.squads??[]).reduce((sum,row)=>sum+row.recent_acts,0)/Math.max(1,windowMs/60000)*10)/10;

export const ARMY_LABELS={
  army:['Армия','The army'],
  where:['Где исполняется','Where it runs'],
  atBridge:['журнал у моста на этой машине','the journal at the bridge on this machine'],
  inBrowser:['журнал в этом браузере','the journal in this browser'],
  agentsNow:['агентов {0}, за 15 минут работали {1}, застряли {2}','{0} agents, {1} worked in the last 15 minutes, {2} stalled'],
  flow:['поток: {0} действий в минуту','flow: {0} acts a minute'],
  queue:['ждут рассмотрения: {0}; темп падает с {1}, остановка на {2}','waiting for review: {0}; pace slows from {1}, stops at {2}'],
  pause:['Остановить всех агентов','Stop every agent'],
  resume:['Продолжить работу агентов','Let the agents go on'],
  paused:['Агенты остановлены: мост не принимает их действий, чтение открыто.','The agents are stopped: the bridge takes none of their acts; reading stays open.'],
  pauseHere:['Остановить: агентам только чтение','Stop: agents may only read'],
  squads:['Отряды','Squads'],
  noSquads:['Отрядов пока нет: агенты работают без отряда','No squads yet: agents work without one'],
  role:['роль','role'],
  undeclared:['не объявлен','not declared'],
  spent:['израсходовано {0} из {1}','spent {0} of {1}'],
  overBudget:['бюджет исчерпан','budget spent'],
  squadActs:['действий: {0}','acts: {0}'],
  accepted:['принято {0}','accepted {0}'],
  open:['ждут {0}','waiting {0}'],
  stalled:['застряли {0}','stalled {0}'],
  assigned:['поручено {0}','assigned {0}'],
  working:['в работе {0} из {1}','at work {0} of {1}'],
  newSquad:['Новый отряд','New squad'],
  squadName:['Имя отряда','Squad name'],
  area:['Область: адреса через пробел','Area: addresses, separated by spaces'],
  budget:['Бюджет, действий','Budget, acts'],
  stop:['Правило остановки','Stop rule'],
  stopDefault:['когда в области нет открытого тикета','when the area has no open ticket'],
  declare:['Объявить отряд','Declare the squad'],
  tier:['Что агентам можно','What agents may do'],
  'tier-read':['Читать','Read'],
  'tier-draft':['Готовить черновики','Prepare drafts'],
  'tier-propose':['Ставить предложения','Make proposals'],
};

const fill=(text,...values)=>values.reduce((out,value,index)=>out.replace(`{${index}}`,String(value)),text);

/**
 * The console's section for the board. `t` reads ARMY_LABELS; `actDesk` keeps the tier; `target` gives the chosen
 * place for a new squad's area; `repaint` draws the board again after a change.
 */
export async function renderArmy({doc,acts,actDesk=null,t,now=Date.now(),windowMs,target=()=>null,repaint=()=>{},onReplay=null,roleName=role=>role.name?.en??role.id}){
  const el=(tag,cls='',text)=>{const node=doc.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=text;return node;};
  const button=(text,cls='text-button')=>{const node=el('button',cls,text);node.type='button';return node;};
  const [squads,status]=await Promise.all([
    acts.query({view:'squads',now_ms:now,window_ms:windowMs,limit:200}).catch(()=>null),
    acts.bridgeStatus?.().catch(()=>null)??null,
  ]);
  const bridge=acts.tier==='bridge'&&status;
  const box=el('section','ab-section ab-army');box.append(el('h3','ad-minor',t('army')));
  box.append(el('p','ab-line-wide',`${t('where')}: ${bridge?t('atBridge'):t('inBrowser')}`));
  if(squads){
    box.append(el('p','ab-line-wide',fill(t('agentsNow'),squads.agents,squads.active,squads.stalled)));
    box.append(el('p','ab-line-wide',fill(t('flow'),flowOf(squads,windowMs).toLocaleString(doc.documentElement.lang||undefined))));
    const open=bridge?status.queue.open:squads.open_proposals;
    box.append(el('p','ab-line-wide',bridge?fill(t('queue'),open,status.queue.soft,status.queue.max):fill(t('open'),open)));
  }
  // The person's controls: the agents' tier and, at the bridge, one pause for all.
  const controls=el('div','ad-row ab-controls');
  const tier=bridge?status.agent_tier:actDesk?.tier?.()??'propose';
  const group=el('div','ad-tiers');group.setAttribute('role','radiogroup');group.setAttribute('aria-label',t('tier'));
  for(const value of ['read','draft','propose']){
    const label=el('label','ad-check-row'),input=el('input');input.type='radio';input.name='ab-tier';input.value=value;input.checked=tier===value;
    input.addEventListener('change',async()=>{actDesk?.setTier?.(value);if(bridge&&!actDesk)await acts.control({agent_tier:value}).catch(()=>{});repaint();});
    label.append(input,el('span','',t(`tier-${value}`)));group.append(label);
  }
  if(bridge){
    const pause=button(status.paused?t('resume'):t('pause'),status.paused?'primary':'text-button ab-stop');pause.dataset.army='pause';
    pause.addEventListener('click',async()=>{await acts.control({paused:!status.paused}).catch(()=>{});repaint();});
    controls.append(pause);
    if(status.paused)box.append(el('p','ab-paused',t('paused')));
  }else{
    const stop=button(t('pauseHere'),'text-button ab-stop');stop.dataset.army='pause';stop.disabled=tier==='read';
    stop.addEventListener('click',()=>{actDesk?.setTier?.('read');repaint();});controls.append(stop);
  }
  box.append(el('p','ad-minor',t('tier')),group,controls);

  // Squads.
  const rows=armyRows(squads);
  const list=el('section','ab-section');list.append(el('h3','ad-minor',`${t('squads')} · ${rows.length}`));
  if(!rows.length)list.append(el('p','ad-muted',t('noSquads')));
  for(const row of rows){
    const line=el('div','ab-squad');line.dataset.squad=row.squad;if(row.over_budget)line.dataset.over='true';
    const facts=[
      row.role?`${t('role')}: ${row.role}`:t('undeclared'),
      fill(t('working'),row.active,row.agents),
      row.budget_acts?fill(t('spent'),row.acts,row.budget_acts):fill(t('squadActs'),row.acts),
      row.accepted_share==null?null:fill(t('accepted'),`${Math.round(row.accepted_share*100)}%`),
      row.proposals.open?fill(t('open'),row.proposals.open):null,
      row.assigned?fill(t('assigned'),row.assigned):null,
      row.stalled?fill(t('stalled'),row.stalled):null,
      row.over_budget?t('overBudget'):null,
    ].filter(Boolean);
    line.append(el('strong','',row.squad),el('span','ab-meta',facts.join(' · ')));
    if(onReplay){const play=button('▶','text-button ab-replay');play.title=t('replay');play.setAttribute('aria-label',`${t('replay')}: ${row.squad}`);play.addEventListener('click',()=>onReplay(row.squad));line.append(play);}
    list.append(line);
  }
  // A new squad: name, role, area, budget and stop rule; the squad act is the person's.
  const form=el('details','ab-new-squad'),summary=el('summary','',t('newSquad'));form.append(summary);
  const name=el('input','ad-input');name.placeholder=t('squadName');name.setAttribute('aria-label',t('squadName'));
  const role=el('select','ad-input');role.setAttribute('aria-label',t('role'));
  for(const item of roles.roles){const option=el('option','',roleName(item));option.value=item.id;role.append(option);}
  const area=el('input','ad-input');area.placeholder=t('area');area.setAttribute('aria-label',t('area'));area.value=target()??'';
  const budget=el('input','ad-input');budget.type='number';budget.min='1';budget.value='200';budget.setAttribute('aria-label',t('budget'));
  const stop=el('input','ad-input');stop.value=t('stopDefault');stop.setAttribute('aria-label',t('stop'));
  const declare=button(t('declare'),'primary');
  declare.addEventListener('click',async()=>{
    const id=name.value.trim().toLowerCase().replace(/[^a-z0-9._-]+/g,'-').replace(/^-+|-+$/g,'');
    const places=area.value.split(/\s+/).filter(Boolean);
    if(!id||!places.length)return;
    await acts.act({kind:'squad',targets:[`squad:${id}`],intent:`${t('newSquad')}: ${id}`,
      body:{role:role.value,area:places,budget:{acts:Math.max(1,Number.parseInt(budget.value,10)||1)},stop:stop.value.trim()||t('stopDefault')}});
    repaint();
  });
  const field=(label,input)=>{const wrap=el('label','ab-field');wrap.append(el('span','ad-muted',label),input);return wrap;};
  form.append(field(t('squadName'),name),field(t('role'),role),field(t('area'),area),field(t('budget'),budget),field(t('stop'),stop),declare);
  list.append(form);
  return {section:[box,list],squads,status};
}
