import '../observatory/human-forms-wasm-test-runtime.mjs';
import {expect,test} from 'vitest';
import {Window} from 'happy-dom';
import {ARMY_LABELS,armyRows,flowOf,renderArmy} from './army-console.mjs';
import {createActJournal} from '../act-journal/journal.mjs';
import {createMemoryActStore} from '../act-journal/store.mjs';
import {simulateArmy} from '../../fixtures/army-simulator.mjs';

const t=key=>ARMY_LABELS[key][1];

test('squads read with their accepted share and spend against the budget',()=>{
  const rows=armyRows({squads:[{squad:'s',acts:150,budget_acts:100,recent_acts:30,proposals:{open:2,accepted:3,rejected:1,deferred:0}},{squad:'u',acts:4,budget_acts:null,recent_acts:0,proposals:{open:0,accepted:0,rejected:0,deferred:0}}]});
  expect(rows[0]).toMatchObject({accepted_share:.75,spend:1.5});
  expect(rows[1]).toMatchObject({accepted_share:null,spend:null});
  expect(flowOf({squads:rows},15*60*1000)).toBe(2);
});

test('the console shows the army by squad and the person stops it at the bridge',async()=>{
  const window=new Window({url:'http://127.0.0.1:8742/zarathustra/'}),doc=window.document;
  const journal=createActJournal({store:createMemoryActStore(),locks:null,channel:null});
  const start=Date.now()-120000;
  await journal.appendMany(simulateArmy({agents:30,squads:3,acts:400,seed:3,start,stepMs:200}).acts);
  let status={schema:'tos.bridge.v1',paused:false,agent_tier:'propose',queue:{open:5,soft:200,max:2000}};
  const changes=[];
  const acts={...journal,query:journal.query,act:journal.act,tier:'bridge',bridgeStatus:async()=>status,control:async change=>{changes.push(change);status={...status,...change};return status;}};
  let painted=0;
  const draw=async()=>{const out=await renderArmy({doc,acts,t,windowMs:15*60*1000,repaint:()=>{painted++;}});const root=doc.createElement('div');root.append(...out.section);return {root,...out};};
  const {root,squads}=await draw();
  expect(squads.squads_count).toBe(3);
  expect(root.querySelectorAll('.ab-squad').length).toBe(3);
  expect(root.textContent).toContain('the journal at the bridge on this machine');
  expect(root.textContent).toContain('waiting for review: 5');
  root.querySelector('[data-army="pause"]').click();await Promise.resolve();await Promise.resolve();
  expect(changes).toEqual([{paused:true}]);
  const again=await draw();
  expect(again.root.textContent).toContain('The agents are stopped');
  expect(again.root.querySelector('[data-army="pause"]').textContent).toBe('Let the agents go on');
  // A squad is the person's act: its role, area, budget and stop rule.
  const form=again.root.querySelector('.ab-new-squad');
  form.querySelector('input').value='Translators';
  form.querySelectorAll('input')[1].value='affirmation:link:01:child';
  form.querySelector('button').click();
  await new Promise(resolve=>setTimeout(resolve,20));
  const fold=await journal.query({view:'squads',now_ms:Date.now(),window_ms:60000});
  expect(fold.squads.find(row=>row.squad==='translators')).toMatchObject({declared:true,role:'reader',budget_acts:200});
  expect(painted).toBeGreaterThan(0);
  journal.dispose();
});

test('in the browser, stopping the agents leaves them reading',async()=>{
  const window=new Window({url:'http://localhost/zarathustra/'}),doc=window.document;
  const journal=createActJournal({store:createMemoryActStore(),locks:null,channel:null});
  let tier='propose';const actDesk={tier:()=>tier,setTier:value=>{tier=value;}};
  const out=await renderArmy({doc,acts:journal,actDesk,t,windowMs:60000});
  const root=doc.createElement('div');root.append(...out.section);
  expect(root.textContent).toContain('the journal in this browser');
  root.querySelector('[data-army="pause"]').click();
  expect(tier).toBe('read');
  journal.dispose();
});
