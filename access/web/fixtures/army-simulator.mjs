// A deterministic stream of act records from one person and an army of agents, for checking the act journal and
// the interface at 1, 50 and 500 agents before such an army exists. Same seed, same stream. Development fixture
// only: these records are synthetic and never reach a stand, its bonsai or the Tree.
import {draftAct} from '../src/act-journal/drafts.mjs';

function mulberry32(seed){
  let state=seed>>>0;
  return ()=>{state=(state+0x6d2b79f5)>>>0;let t=state;t=Math.imul(t^(t>>>15),t|1);t^=t+Math.imul(t^(t>>>7),t|61);return ((t^(t>>>14))>>>0)/4294967296;};
}

const ROLES=['reader','philologist','link-finder','opponent','quote-checker','guide'];
const MODELS=['model-a','model-b','model-c','local-small'];
const WORDS=[['дитя','ребёнок'],['святое слово утверждения','священное «да»'],['сверхчеловек','сверх-человек'],['мгновенье','мгновение']];
const DEFAULT_TARGETS=Array.from({length:400},(_,n)=>`sim:link:${n}`).concat(Array.from({length:81},(_,n)=>`sim:chapter:${n}`));

/**
 * `agents` agents in `squads` squads act on `targets` for `acts` records starting at `start`, `stepMs` apart.
 * The mix follows a working session: agents read (notes), take leases, propose and reply; the person files
 * tickets, assigns them and reviews groups. `duplicates` is the share of proposals that repeat a change already
 * made; `collisions` the share of leases taken on a target another agent holds.
 */
export function simulateArmy({agents=50,squads=Math.max(1,Math.ceil(agents/25)),acts=2000,seed=1,start=Date.UTC(2026,10,20,9),stepMs=50,
  targets=DEFAULT_TARGETS,duplicates=.3,collisions=.05,person={type:'human',id:'sim-person'}}={}){
  const random=mulberry32(seed),pick=items=>items[Math.floor(random()*items.length)];
  const army=Array.from({length:agents},(_,n)=>({type:'agent',id:`sim-agent-${n}`,model:MODELS[n%MODELS.length],role:ROLES[n%ROLES.length],squad:`sim-squad-${n%squads}`}));
  const out=[],tickets=[],proposals=[],held=new Map();
  let clock=start,sequence=0,reviewed=0;
  const add=fields=>{const record=draftAct({...fields,id:`sim-${seed}-${++sequence}`,at:new Date(clock)});out.push(record);clock+=Math.max(1,Math.round(stepMs*(.5+random())));return record;};
  // Squads are declared first: role, area, budget, stop rule.
  for(let n=0;n<squads;n++)add({kind:'squad',author:person,targets:[`squad:sim-squad-${n}`],intent:`Squad ${n}`,
    body:{role:ROLES[n%ROLES.length],area:targets.slice((n*20)%targets.length,(n*20)%targets.length+20),budget:{acts:Math.ceil(acts/squads)},stop:'when the area has no open ticket'}});
  while(out.length<acts){
    const roll=random();
    if(roll<.04||!tickets.length){
      const target=pick(targets),words=pick(WORDS);
      const ticket=add({kind:'ticket',author:person,targets:[target],intent:`Перевод: «${words[0]}»`,body:{category:'translation',variant:'outdated',observation:`«${words[0]}» устарело; слово Древа «${words[1]}»`}});
      tickets.push({id:ticket.id,target,words});
      continue;
    }
    if(roll<.07){
      const ticket=pick(tickets);
      add({kind:'assignment',author:person,targets:[`act:${ticket.id}`],intent:'Поручить отряду',body:{assignee:`squad:${pick(army).squad}`}});
      continue;
    }
    if(roll<.09&&proposals.length>reviewed){
      // The person decides the oldest open proposals together, up to 20 at a time.
      const batch=proposals.slice(reviewed,reviewed+20);reviewed+=batch.length;
      add({kind:'review',author:person,targets:batch.map(id=>`act:${id}`),intent:'Решение по предложениям',body:{decision:random()<.8?'accept':'defer',reason:'checked against the places'}});
      continue;
    }
    if(!agents){clock+=stepMs;continue;}
    const agent=pick(army);
    if(roll<.25){
      const target=pick(targets);
      const contested=random()<collisions&&held.size?pick([...held.keys()]):target;
      add({kind:'lease',author:agent,targets:[contested],intent:'Работаю над местом',body:{ttl_ms:60000}});
      held.set(contested,agent.id);
      continue;
    }
    if(roll<.32&&held.size){
      const target=pick([...held.keys()]);
      add({kind:'release',author:agent,targets:[target],intent:'Закончил',body:{}});held.delete(target);
      continue;
    }
    if(roll<.62){
      const ticket=pick(tickets);
      const repeat=random()<duplicates;
      const words=repeat?ticket.words:pick(WORDS);
      const record=add({kind:'proposal',author:agent,targets:[repeat?ticket.target:pick(targets)],grounds:[`sim:place:${Math.floor(random()*775)}`],parents:[ticket.id],
        intent:`«${words[0]}» → «${words[1]}»`,body:{category:'translation',variant:'outdated',change:{field:'verb',from:words[0],to:words[1]}}});
      proposals.push(record.id);
      continue;
    }
    if(roll<.72){
      const ticket=pick(tickets);
      add({kind:'reply',author:agent,targets:[ticket.target],grounds:[`sim:place:${Math.floor(random()*775)}`],parents:[ticket.id],intent:'Прочитал место',body:{text:'Место прочитано; основания приложены.'}});
      continue;
    }
    add({kind:'note',author:agent,targets:[pick(targets)],intent:'Заметка при чтении',body:{text:'Заметка агента.'}});
  }
  return {acts:out.slice(0,acts),proposals,tickets:tickets.map(ticket=>ticket.id)};
}
