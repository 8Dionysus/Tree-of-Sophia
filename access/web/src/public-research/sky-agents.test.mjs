import '../observatory/human-forms-wasm-test-runtime.mjs';
import {expect,test} from 'vitest';
import {Window} from 'happy-dom';
import {mountSkyAgents,presenceMap,SKY_AGENTS_KEY} from './sky-agents.mjs';
import {createActJournal} from '../act-journal/journal.mjs';
import {createMemoryActStore} from '../act-journal/store.mjs';
import {draftAct} from '../act-journal/drafts.mjs';

test('presence becomes a map of stars to agents and their roles, the most active first',()=>{
  const map=presenceMap({targets:[{target:'III.02',agents:3,roles:[{role:'reader',acts:1},{role:'opponent',acts:4}]},{target:'x',agents:1,roles:[{role:null,acts:1}]}]});
  expect(map.get('III.02')).toEqual({count:3,roles:['opponent','reader']});
  expect(map.get('x')).toEqual({count:1,roles:[]});
});

test('the agents sky marks where agents work, names its marks and keeps the choice',async()=>{
  const window=new Window({url:'http://localhost/zarathustra/'}),doc=window.document;
  const journal=createActJournal({store:createMemoryActStore(),locks:null,channel:null});
  const agent=(id,role)=>({type:'agent',id,model:'m',role});
  await journal.appendMany([
    draftAct({kind:'note',author:agent('a1','reader'),targets:['III.02'],intent:'a',body:{text:'t'}}),
    draftAct({kind:'note',author:agent('a2','opponent'),targets:['III.02'],intent:'a',body:{text:'t'}}),
    draftAct({kind:'note',author:agent('a3','reader'),targets:['III.13'],intent:'a',body:{text:'t'}}),
  ]);
  const slot=doc.createElement('div'),seen=[],said=[],actions=[];
  const space={actions:{agentsLegend:()=>slot,setAgentPresence:value=>{seen.push(value);return Boolean(value);}}};
  const storage=new Map();const store={getItem:key=>storage.get(key)??null,setItem:(key,value)=>storage.set(key,value)};
  const sky=mountSkyAgents({actionLayer:{addActions:list=>{actions.push(...list);return ()=>{};},say:text=>said.push(text)},acts:journal,space,locale:()=> 'en',storage:store,view:window});
  expect(actions.map(item=>item.id)).toEqual(['sky-agents']);
  await sky.set(true);
  const map=seen.at(-1);
  expect(map.get('III.02')).toEqual({count:2,roles:expect.arrayContaining(['reader','opponent'])});
  expect(map.get('III.13').count).toBe(1);
  expect(slot.hidden).toBe(false);
  expect(slot.textContent).toContain('A ring: several agents');
  expect(storage.get(SKY_AGENTS_KEY)).toBe('on');
  await sky.set(false);
  expect(seen.at(-1)).toBeNull();
  expect(slot.hidden).toBe(true);
  expect(said).toEqual(['Agents are shown on the sky','Agents are hidden from the sky']);
  sky.destroy();journal.dispose();
});
