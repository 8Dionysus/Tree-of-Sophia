import '../observatory/human-forms-wasm-test-runtime.mjs';
import {expect,test,vi} from 'vitest';
import {Window} from 'happy-dom';
import {CATEGORIES,categorySets,mountHighlight} from './highlight-categories.mjs';
import {createActJournal} from '../act-journal/journal.mjs';
import {createMemoryActStore} from '../act-journal/store.mjs';
import links from '../../../../bonsai/zarathustra/core/links.json' with {type:'json'};

test('each category reads one recorded property of the stand or the journal',()=>{
  const core={links};
  const verdict=categorySets('verdict',{core});
  expect(verdict.channel).toBe('links');
  expect(verdict.edges.length).toBe(links.filter(link=>['revise','reconstruction','rejected'].includes(link.assessment.verdict)).length);
  expect(categorySets('counter',{core}).edges.every(id=>links.find(link=>link.id===id).assessment.counter_reading)).toBe(true);
  expect(categorySets('tickets',{core,tickets:new Set(['child'])})).toEqual({edges:[],nodes:['child'],channel:'stars'});
  expect(categorySets('nothing',{core})).toBeNull();
  expect(CATEGORIES.map(item=>item.id)).toEqual(['verdict','counter','attention','tickets','agents']);
});

test('one category at a time, only in the research layer; the legend names what the channel shows',async()=>{
  const window=new Window({url:'http://localhost/zarathustra/'}),document=window.document;
  const root=document.createElement('div'),slot=document.createElement('div');root.append(slot);document.body.append(root);
  let layer='research';
  const setHighlight=vi.fn(),space={root,actions:{highlightLegend:()=>slot,core:()=>({links}),setHighlight}};
  const acts=createActJournal({store:createMemoryActStore(),locks:null,channel:null});
  await acts.act({kind:'ticket',targets:['affirmation:link:01:child'],grounds:[],intent:'which verb',body:{category:'relation',observation:'which verb'}});
  const added=[];const view=mountHighlight({actionLayer:{layer:()=>layer,addActions:list=>{added.push(...list);return ()=>{};}},acts,space,locale:()=> 'en'});
  expect(slot.hidden).toBe(false);
  expect([...slot.querySelectorAll('.ps-highlight-chip')].map(chip=>chip.textContent)).toEqual(['None','Review verdict','Counter-reading','Review attention','My tickets','Agents at work']);
  await view.set('counter');
  expect(slot.querySelector('.ps-highlight-caption').textContent).toBe('Brightness and line weight: links with a recorded counter-reading');
  expect(slot.querySelectorAll('[aria-pressed=true]')).toHaveLength(1);
  await view.set('tickets');
  expect(setHighlight).toHaveBeenLastCalledWith({edges:['affirmation:link:01:child'],nodes:['affirmation:link:01:child']});
  // Leaving the research layer puts the highlight out.
  layer='reading';root.dispatchEvent(new window.CustomEvent('public-research-layer'));
  expect(setHighlight).toHaveBeenLastCalledWith(null);expect(slot.hidden).toBe(true);
  expect(added.every(action=>action.when()===false)).toBe(true);
  view.destroy();
});
