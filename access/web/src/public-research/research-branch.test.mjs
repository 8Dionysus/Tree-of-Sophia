import '../observatory/human-forms-wasm-test-runtime.mjs';
import {expect,test} from 'vitest';
import {Window} from 'happy-dom';
import {branchAddress,branchesOf,mountResearchBranches} from './research-branch.mjs';
import {createActJournal} from '../act-journal/journal.mjs';
import {createMemoryActStore} from '../act-journal/store.mjs';
import {draftAct} from '../act-journal/drafts.mjs';

test('a branch has an address from its question and its newest act is the branch',()=>{
  expect(branchAddress('Почему дитя — последнее превращение?')).toBe('branch:почему-дитя-последнее-превращение');
  expect(branchAddress('x',new Set(['branch:x']))).toBe('branch:x-2');
  const at=s=>new Date(Date.UTC(2026,10,20,10,0,s)).toISOString();
  const list=branchesOf([
    {id:'b1',kind:'branch',targets:['branch:q'],at:at(1),body:{question:'Q?',tickets:['t1']}},
    {id:'b2',kind:'branch',targets:['branch:q'],at:at(5),body:{question:'Q, sharper?',tickets:['t1','t2'],session:'s1'}},
  ]);
  expect(list).toHaveLength(1);
  expect(list[0]).toMatchObject({id:'b2',question:'Q, sharper?',tickets:['t1','t2'],session:'s1'});
});

test('a branch ties the question to tickets, thoughts, the layout and the session, and keeps its history',async()=>{
  const window=new Window({url:'http://localhost/zarathustra/?item=III.02'}),doc=window.document;doc.body.innerHTML='<div id="tree"></div>';
  const journal=createActJournal({store:createMemoryActStore(),locks:null,channel:null});
  await journal.append(draftAct({id:'t1',kind:'ticket',targets:['III.02'],intent:'word',body:{category:'translation',observation:'o'}}));
  await journal.append(draftAct({kind:'workspace-event',targets:['thought-7'],intent:'Workspace: thought',body:{action:'thought'}}));
  const branches=mountResearchBranches({shell:doc.querySelector('#tree'),actionLayer:{addActions:()=>()=>{},say:()=>{},linkTo:()=>'http://localhost/zarathustra/?item=III.02&layer=research',currentTarget:()=>({id:'III.02'})},
    acts:journal,api:{getQuestion:()=>'Why is the child last?'},getSession:()=>'session-1',locale:()=> 'en',view:window});
  const dialog=await branches.open();
  const editor=dialog.querySelector('.rb-editor');
  expect(editor.querySelector('input').value).toBe('Why is the child last?');
  editor.querySelector('input[data-group="ticket"]').checked=true;
  editor.querySelector('input[data-group="thought"]').checked=true;
  editor.querySelector('button.primary').click();
  await new Promise(resolve=>setTimeout(resolve,30));
  const card=dialog.querySelector('[data-branch]');
  expect(card.dataset.branch).toBe('branch:why-is-the-child-last');
  const [row]=await journal.recent({limit:1});
  expect(row.record).toMatchObject({kind:'branch',parents:['t1'],body:{question:'Why is the child last?',tickets:['t1'],thoughts:['thought-7'],layout:'http://localhost/zarathustra/?item=III.02&layer=research',session:'session-1'}});
  // A second save replaces the first; the branch keeps one address.
  card.querySelectorAll('.ad-row button')[1].click();
  dialog.querySelector('[data-branch] .rb-editor input').value='Why is the child the last metamorphosis?';
  dialog.querySelector('[data-branch] .rb-editor button.primary').click();
  await new Promise(resolve=>setTimeout(resolve,30));
  expect(dialog.querySelectorAll('[data-branch]')).toHaveLength(1);
  const [latest]=await journal.recent({limit:1});
  expect(latest.record.replaces).toBe(row.record.id);
  const history=await journal.query({view:'target',address:'branch:why-is-the-child-last'});
  expect(history.count).toBe(2);
  branches.destroy();journal.dispose();
});
