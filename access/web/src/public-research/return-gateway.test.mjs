import '../observatory/human-forms-wasm-test-runtime.mjs';
import {expect,test} from 'vitest';
import {Window} from 'happy-dom';
import {mountReturnGateway,transfersOf} from './return-gateway.mjs';
import {createActJournal} from '../act-journal/journal.mjs';
import {createMemoryActStore} from '../act-journal/store.mjs';
import {draftAct} from '../act-journal/drafts.mjs';

test('transfers carry their returns and their state',()=>{
  const at=s=>new Date(Date.UTC(2026,10,20,10,0,s)).toISOString();
  const list=transfersOf([
    {id:'t1',kind:'transfer',targets:['III.02'],body:{format:'agent',recipient:'clipboard'},at:at(1),intent:'x'},
    {id:'t2',kind:'transfer',targets:['III.13'],body:{format:'quote',recipient:'clipboard'},at:at(2),intent:'y'},
    {id:'r1',kind:'return',parents:['t1'],targets:['III.02'],body:{text:'answer'},at:at(3)},
  ]);
  expect(list.map(item=>[item.id,item.state,item.returns.length])).toEqual([['t2','sent',0],['t1','returned',1]]);
});

test('a result brought back stands at the place the hand-over left from',async()=>{
  const window=new Window({url:'http://localhost/zarathustra/'}),doc=window.document;doc.body.innerHTML='<div id="tree"></div>';
  const journal=createActJournal({store:createMemoryActStore(),locks:null,channel:null});
  await journal.append(draftAct({id:'t1',kind:'transfer',targets:['III.02'],intent:'Task for an agent',body:{format:'agent',recipient:'clipboard',bytes:10}}));
  const opened=[];
  const gateway=mountReturnGateway({shell:doc.querySelector('#tree'),actionLayer:{addActions:()=>()=>{},say:()=>{}},acts:journal,
    space:{index:{get:id=>id==='III.02'?{title:{ru:'О детях',en:'On children'}}:null}},experience:{open:id=>opened.push(id)},locale:()=> 'en'});
  const dialog=await gateway.open();
  const card=dialog.querySelector('[data-transfer="t1"]');
  expect(card.dataset.state).toBe('sent');
  expect(card.querySelector('h3').textContent).toBe('On children');
  card.querySelector('textarea').value='The place says «Kind»; the Tree\'s word is «child».';
  card.querySelector('button.primary').click();
  await new Promise(resolve=>setTimeout(resolve,20));
  const fold=await journal.query({view:'target',address:'III.02'});
  expect(JSON.stringify(fold)).toContain('return');
  expect(dialog.querySelector('[data-transfer="t1"]').dataset.state).toBe('returned');
  expect(dialog.querySelector('.rg-return').textContent).toContain('child');
  dialog.querySelector('[data-transfer="t1"] .ad-row button').click();
  expect(opened).toEqual(['III.02']);
  gateway.destroy();journal.dispose();
});
