import '../observatory/human-forms-wasm-test-runtime.mjs';
import {expect,test,vi} from 'vitest';
import {Window} from 'happy-dom';
import {importActLines,mountCollect,unsorted} from './collect.mjs';
import {actLines} from './device-handoff.mjs';
import {createActJournal} from '../act-journal/journal.mjs';
import {createMemoryActStore} from '../act-journal/store.mjs';

const journal=()=>createActJournal({store:createMemoryActStore(),locks:null,channel:null});

function setup({coarse=true,acts=journal()}={}){
  const window=new Window({url:'http://localhost/zarathustra/'}),document=window.document;document.body.innerHTML='<div id="tree"></div>';
  const added=[],actionLayer={addActions:list=>{added.push(...list);return ()=>{};},say:vi.fn(),currentTarget:()=>({type:'star',id:'child',label:'The child'})};
  const actDesk={openTicket:vi.fn(async()=>{})};
  const media=()=>({matches:coarse,addEventListener(){},removeEventListener(){}});
  const space={index:{get:id=>id==='child'?{id,kind:'conception',title:{en:'The child'}}:null}};
  const collector=mountCollect({shell:document.querySelector('#tree'),actionLayer,acts,actDesk,space,locale:()=> 'en',media});
  return {window,document,acts,added,actionLayer,actDesk,collector};
}
const pressed=(document,text)=>[...document.querySelectorAll('button')].find(node=>node.textContent===text);

test('a phone collects with a touch and a sentence: a note of the person, marked as collected, on what is in front of it',async()=>{
  const {document,acts,added}=setup();
  expect(added.map(action=>action.id)).toEqual(['collect','inbox']);
  expect(document.querySelector('.cl-fab').hidden).toBe(false);
  document.querySelector('.cl-fab').click();
  expect(document.querySelector('.ad-subject').textContent).toBe('The child');
  pressed(document,'translation').click();
  const text=document.querySelector('.cl-text');text.value='the 1913 word for Kind';
  pressed(document,'Save').click();
  await vi.waitFor(async()=>expect((await acts.recent()).length).toBe(1));
  const [{record}]=await acts.recent();
  expect(record).toMatchObject({kind:'note',targets:['child'],author:{type:'human'},body:{text:'the 1913 word for Kind',collected:true,device:'touch',tag:'translation'}});
});

test('a large screen has no button of its own; the palette reaches both',()=>{
  const {document}=setup({coarse:false});
  expect(document.querySelector('.cl-fab').hidden).toBe(true);
});

test('sorting: a note becomes a ticket that keeps it as a parent, or is let go; either way it leaves the collected',async()=>{
  const {document,acts,actDesk,collector}=setup({coarse:false});
  const one=await acts.act({kind:'note',targets:['child'],grounds:[],intent:'a',body:{text:'a',collected:true,tag:'translation'}});
  await acts.act({kind:'note',targets:['child'],grounds:[],intent:'b',body:{text:'b',collected:true}});
  await acts.act({kind:'note',targets:['child'],grounds:[],intent:'c',body:{text:'c'}});
  await collector.openInbox();
  expect(document.querySelectorAll('.cl-note')).toHaveLength(2);
  const first=[...document.querySelectorAll('.cl-note')].find(card=>card.textContent.includes('translation'));
  [...first.querySelectorAll('button')].find(node=>node.textContent==='To a ticket').click();
  expect(actDesk.openTicket).toHaveBeenCalledWith(expect.objectContaining({type:'star',id:'child'}),{category:'translation',observation:'a',parents:[one.record.id]});
  // The ticket the desk files keeps the note as its parent.
  await acts.act({kind:'ticket',targets:['child'],grounds:[],parents:[one.record.id],intent:'a',body:{category:'translation',observation:'a'}});
  const left=[...document.querySelectorAll('.cl-note')].find(card=>!card.textContent.includes('translation'));
  [...left.querySelectorAll('button')].find(node=>node.textContent==='Let go').click();
  await vi.waitFor(async()=>expect(unsorted((await acts.recent()).map(row=>row.record))).toHaveLength(0));
});

test('what a phone collected arrives through its journal: new acts are added, acts already here are kept as they are',async()=>{
  const phone=journal(),desk=journal();
  await phone.act({kind:'note',targets:['child'],grounds:[],intent:'on the tram',body:{text:'on the tram',collected:true}});
  const shared=await phone.act({kind:'note',targets:['child'],grounds:[],intent:'twice',body:{text:'twice',collected:true}});
  await desk.append(shared.record);
  const lines=await actLines(phone);
  const result=await importActLines(desk,`${lines}not json\n`);
  expect(result).toMatchObject({added:1,present:1,refused:1});
  expect(unsorted((await desk.recent()).map(row=>row.record)).map(record=>record.intent).sort()).toEqual(['on the tram','twice']);
  // A record changed on the way is refused by its digest.
  const tampered=JSON.parse(lines.split('\n')[0]);tampered.intent='changed';tampered.id=`${tampered.id}x`;
  expect((await importActLines(journal(),JSON.stringify(tampered))).refused).toBe(1);
});
