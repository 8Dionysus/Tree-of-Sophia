import '../observatory/human-forms-wasm-test-runtime.mjs';
import {expect,test,vi} from 'vitest';
import {Window} from 'happy-dom';
import {mountActDesk} from './act-desk.mjs';
import {createActJournal} from '../act-journal/journal.mjs';
import {createMemoryActStore} from '../act-journal/store.mjs';

const record={id:'affirmation:link:01:child',verb:{ru:'одно из имён дитяти',en:'is one of the child’s names'},
  grounds:[{w:'also-sprach-zarathustra',c:'I.02',u:['tos.text-unit.sid-78b5795aca827182f53ba266a3271646','tos.text-unit.sid-78b5795aca827182f53ba266a3271646'],q:'Unschuld ist das Kind'}]};
const terms=[{id:'child',original:{lemma:'Kind'},renderings:{ru:{tos:{term:'ребёнок'}}}}];

function setup(){
  const window=new Window({url:'http://localhost/zarathustra/'});
  const document=window.document;document.body.innerHTML='<div id="tree"></div>';
  const shell=document.querySelector('#tree');
  const item={id:record.id,kind:'relation',title:record.verb,record};
  const space={index:{revision:'r1',get:id=>id===record.id?item:null},actions:{terms:()=>terms}};
  const acts=createActJournal({store:createMemoryActStore(),locks:null,channel:null});
  const added=[];const actionLayer={addActions:list=>{added.push(...list);return ()=>{};},say:vi.fn(),copy:vi.fn(async()=>true),linkTo:()=>'http://localhost/zarathustra/?item=x'};
  const storage=new Map();const store={getItem:key=>storage.get(key)??null,setItem:(key,value)=>storage.set(key,value)};
  const desk=mountActDesk({shell,actionLayer,space,acts,registry:null,api:{getQuestion:()=>''},locale:()=> 'en',profile:'zarathustra',storage:store});
  return {window,document,acts,desk,added,actionLayer,target:{type:'link',id:record.id,label:'one of the child’s names',item}};
}
const set=(document,id,value)=>{const node=document.querySelector(`#${id}`);node.value=value;node.dispatchEvent(new document.defaultView.Event(node.tagName==='SELECT'?'change':'input'));};

test('the desk adds its actions to the menu',()=>{
  const {added}=setup();
  expect(added.map(action=>action.id)).toEqual(['ticket','transfer','board','review','connect']);
});

test('a ticket with a proposal is checked on the device, saved, decided as a group and gathered into a packet',async()=>{
  const {document,acts,desk,target}=setup();
  await desk.openTicket(target);
  set(document,'ad-category','translation');set(document,'ad-variant','outdated');
  set(document,'ad-observation','the 1913 word; the Tree says ребёнок');
  const toggle=document.querySelector('#ad-propose');toggle.checked=true;toggle.dispatchEvent(new document.defaultView.Event('change'));
  set(document,'ad-field','verb');set(document,'ad-change-from','дитяти');set(document,'ad-change-to','ребёнка');set(document,'ad-term','child');
  expect(document.querySelector('#ad-to-lemma').value).toBe('ребёнок');
  const levels=[...document.querySelectorAll('.ad-check')].map(row=>row.dataset.level);
  expect(levels.every(level=>level==='ok')).toBe(true);
  document.querySelector('.ad-save').click();
  await vi.waitFor(async()=>expect((await acts.query({view:'summary'})).acts).toBe(2));
  const review=await acts.query({view:'review'});
  expect(review.groups[0]).toMatchObject({category:'translation',open_count:1});
  await desk.openReview();
  await vi.waitFor(()=>expect(document.querySelector('.ad-group')).not.toBeNull());
  expect(document.querySelector('.ad-group h3').textContent).toContain('Kind → ребёнок');
  const reason=document.querySelector('.ad-group input');reason.value='the Tree\'s word for Kind';
  document.querySelector('.ad-group .primary').click();
  await vi.waitFor(async()=>expect((await acts.query({view:'decided'})).unpacked).toBe(1));
  await vi.waitFor(()=>expect(document.querySelector('.ad-packet .primary')).not.toBeNull());
  document.querySelector('.ad-packet .primary').click();
  await vi.waitFor(async()=>expect((await acts.query({view:'decided'})).unpacked).toBe(0));
  const board=await acts.query({view:'board'});
  expect(board.tickets[0].status).toBe('reviewed');
});

test('the agent tier is the person\'s choice and the instruction carries the rules and roles',async()=>{
  const {document,desk}=setup();
  expect(desk.tier()).toBe('propose');
  desk.openConnect();
  const draft=document.querySelector('#ad-tier-draft');draft.checked=true;draft.dispatchEvent(new document.defaultView.Event('change'));
  expect(desk.tier()).toBe('draft');
  expect(desk.instruction()).toContain('Tier: draft.');
  expect(desk.instruction()).toContain('Philologist of the word (philologist)');
});
