import '../observatory/human-forms-wasm-test-runtime.mjs';
import {expect,test,vi} from 'vitest';
import {Window} from 'happy-dom';
import {agentsOf,mountAgentsBoard} from './agents-board.mjs';
import {createActJournal} from '../act-journal/journal.mjs';
import {createMemoryActStore} from '../act-journal/store.mjs';
import {simulateArmy} from '../../fixtures/army-simulator.mjs';

test('agents are read from the newest records: model, role, squad, acts in the window, last place',()=>{
  const now=Date.parse('2026-11-20T10:00:00Z'),at=s=>new Date(now-s*1000).toISOString();
  const agent=(id,role)=>({type:'agent',id,model:'model-a',role,squad:'s1'});
  const rows=[
    {kind:'note',author:agent('a1','reader'),targets:['x'],at:at(30)},
    {kind:'proposal',author:agent('a1','philologist'),targets:['y'],at:at(10)},
    {kind:'note',author:agent('a2','reader'),targets:['z'],at:at(2000)},
    {kind:'note',author:{type:'human',id:'local-person'},targets:['x'],at:at(5)},
  ];
  expect(agentsOf(rows,{now})).toEqual([{id:'a1',model:'model-a',role:'philologist',squad:'s1',acts:2,last:now-10000,target:'y',kinds:{note:1,proposal:1}}]);
});

test('the board shows the agents, the places they work on, the places held and what waits for review',async()=>{
  const window=new Window({url:'http://localhost/zarathustra/'}),document=window.document;document.body.innerHTML='<div id="tree"></div>';
  const acts=createActJournal({store:createMemoryActStore(),locks:null,channel:null});
  const start=Date.now()-60000;
  const army=simulateArmy({agents:12,squads:3,acts:120,seed:7,start,stepMs:200});
  await acts.appendMany(army.acts);
  const added=[];
  const board=mountAgentsBoard({shell:document.querySelector('#tree'),actionLayer:{addActions:list=>{added.push(...list);return ()=>{};}},acts,locale:()=> 'en',view:window});
  expect(added.map(action=>action.id)).toEqual(['agents-board','agents-board-window']);
  const root=document.createElement('div');document.body.append(root);
  const result=await board.render(root);
  expect(result.agents.length).toBeGreaterThan(0);
  expect(root.querySelectorAll('.ab-agent').length).toBe(Math.min(200,result.agents.length));
  expect(root.textContent).toContain('Waiting for review');
  expect(root.textContent).toContain(`groups: ${result.review.groups.length}`);
  board.destroy();
});
