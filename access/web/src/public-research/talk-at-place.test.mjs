import '../observatory/human-forms-wasm-test-runtime.mjs';
import {expect,test} from 'vitest';
import {Window} from 'happy-dom';
import {mountTalk,threadsOf} from './talk-at-place.mjs';
import {createActJournal} from '../act-journal/journal.mjs';
import {createMemoryActStore} from '../act-journal/store.mjs';

test('a thread keeps the form of a dispute: thesis, grounds, objection, answer',()=>{
  const at=s=>new Date(Date.UTC(2026,10,20,10,0,s)).toISOString();
  const threads=threadsOf([
    {id:'a',at:at(1),body:{move:'thesis',text:'T'}},
    {id:'b',at:at(2),parents:['a'],body:{move:'grounds',text:'G'}},
    {id:'c',at:at(3),parents:['a'],body:{move:'objection',text:'O'}},
    {id:'d',at:at(4),parents:['c'],body:{move:'answer',text:'A'}},
    {id:'e',at:at(5),parents:['a'],body:{move:'answer',text:'not an answer to a thesis'}},
    {id:'f',at:at(6),body:{text:'a plain note'}},
  ]);
  expect(threads).toHaveLength(1);
  expect(threads[0].replies.map(item=>item.move)).toEqual(['grounds','objection']);
  expect(threads[0].replies[1].replies[0].record.id).toBe('d');
});

test('the talk opens at its place and grows move by move, people and agents alike',async()=>{
  const window=new Window({url:'http://localhost/zarathustra/'}),doc=window.document;doc.body.innerHTML='<div id="tree"></div>';
  const journal=createActJournal({store:createMemoryActStore(),locks:null,channel:null});
  const said=[];
  const talk=mountTalk({shell:doc.querySelector('#tree'),actionLayer:{addActions:()=>()=>{},say:text=>said.push(text)},acts:journal,locale:()=> 'en'});
  const thesis=await talk.say('III.02','thesis','The child is the first movement, not the last.');
  await journal.act({kind:'reply',author:{type:'agent',id:'opponent-1',model:'m',role:'opponent'},targets:['III.02'],parents:[thesis.record.id],intent:'objection',body:{text:'The text calls the child a new beginning.',move:'objection'}});
  const dialog=await talk.open({type:'star',id:'III.02',label:'Of the three metamorphoses'});
  expect(dialog.querySelector('h2').textContent).toContain('Of the three metamorphoses');
  const objection=dialog.querySelector('[data-move="objection"]');
  expect(objection.textContent).toContain('opponent-1');
  // Grounds need exact places.
  dialog.querySelector('[data-move="thesis"] > .ad-row button').click();
  const form=dialog.querySelector('[data-move="thesis"] .tk-form');
  form.querySelector('textarea').value='See the closing lines.';
  form.querySelector('button').click();await new Promise(resolve=>setTimeout(resolve,20));
  expect(said).toEqual(['Grounds need exact places']);
  form.querySelector('input').value='place:zarathustra:de:I.01#u12';
  form.querySelector('button').click();await new Promise(resolve=>setTimeout(resolve,30));
  expect([...dialog.querySelectorAll('[data-move]')].map(node=>node.dataset.move)).toEqual(['thesis','objection','grounds']);
  dialog.querySelector('[data-move="objection"] > .ad-row button').click();
  const answer=dialog.querySelector('[data-move="objection"] .tk-form');
  answer.querySelector('textarea').value='A beginning is the first movement.';
  answer.querySelector('button').click();await new Promise(resolve=>setTimeout(resolve,30));
  expect(dialog.querySelector('[data-move="answer"]').textContent).toContain('first movement');
  talk.destroy();journal.dispose();
});
