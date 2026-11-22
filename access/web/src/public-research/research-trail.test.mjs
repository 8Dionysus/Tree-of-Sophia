import '../observatory/human-forms-wasm-test-runtime.mjs';
import {expect,test,vi} from 'vitest';
import {Window} from 'happy-dom';
import {frontierOf,growthBetween,mountResearchTrail,rememberRevision,subjectStages,SEEN_REVISION_KEY} from './research-trail.mjs';
import {createActJournal} from '../act-journal/journal.mjs';
import {createMemoryActStore} from '../act-journal/store.mjs';
import links from '../../../../bonsai/zarathustra/core/links.json' with {type:'json'};
import chains from '../../../../bonsai/zarathustra/core/chains.json' with {type:'json'};
import proposed from '../../../../bonsai/zarathustra/core/proposed.json' with {type:'json'};
import witnesses from '../../../../bonsai/zarathustra/witnesses.json' with {type:'json'};

const ground=c=>({w:'also-sprach-zarathustra',c,u:['tos.text-unit.sid-a','tos.text-unit.sid-a']});
const chapters=['I.01','I.02','I.03','II.01'].map(key=>({key,titles:{ru:`Глава ${key}`,en:`Chapter ${key}`,de:`Kapitel ${key}`}}));
const link=(id,c,extra={})=>({id,from:'a',to:'b',verb:{ru:'ведёт к',en:'leads to'},grounds:[ground(c)],status:'text',admission:'map',assessment:{verdict:'confirmed',reason:'ok',reviewer:'r'},...extra});
const core={
  links:[
    link('plain','I.01'),
    link('built','I.02',{status:'reconstruction',assessment:{verdict:'reconstruction',reason:'no sentence says it',reviewer:'r'}}),
    link('argued','I.02',{assessment:{verdict:'revise',reason:'narrower',reviewer:'r',counter_reading:{ru:'иначе',grounds:[ground('II.01')]}}}),
    link('kept','I.01',{assessment:{verdict:'confirmed',reason:'ok',reviewer:'r',counter_reading:{ru:'иначе'}}}),
    link('dropped','I.01',{admission:'rejected',status:'reconstruction',assessment:{verdict:'rejected',reason:'no',reviewer:'r'}}),
    // A chapter cited only by a former wording is not read by the map as it stands.
    link('old','I.01',{verb_revision_history:[{verb:{ru:'было',en:'was'},grounds:[ground('I.03')]}]}),
  ],
  chains:[],proposed:[{id:'snake',title:{ru:'Змея',en:'Snake'},reading:{grounds:[ground('I.03')]}}],concepts:[],figures:[],
};

test('the frontier puts each link in one open group and finds the chapters no link of thought rests on',()=>{
  const found=frontierOf({core,chapters});
  expect(found.rejected.map(row=>row.id)).toEqual(['dropped']);
  expect(found.reconstructions.map(row=>row.id)).toEqual(['built']);
  expect(found.disputed.map(row=>row.id)).toEqual(['argued']);
  // II.01 is cited only by a counter-reading inside an assessment: it still counts as read by that link.
  expect(found.chapters.map(row=>row.id)).toEqual(['I.03']);
  expect(found.proposed).toMatchObject([{id:'snake',chapter:'I.03'}]);
});

test('on the stand\'s own core every frontier entry keeps to the rule of its group',()=>{
  const found=frontierOf({core:{links,chains,proposed}});
  expect(found.reconstructions.length).toBeGreaterThan(0);
  for(const row of found.reconstructions)expect(['reconstruction','hypothesis']).toContain(row.record.status);
  for(const row of found.disputed)expect(row.record.assessment.verdict).not.toBe('confirmed');
  for(const row of found.rejected)expect(row.record.admission==='rejected'||row.record.assessment.verdict==='rejected').toBe(true);
  const ids=[...found.rejected,...found.reconstructions,...found.disputed].map(row=>row.id);
  expect(new Set(ids).size).toBe(ids.length);
  expect(found.chapters.every(row=>!links.some(item=>item.grounds.some(cite=>cite.c===row.chapter)))).toBe(true);
});

test('growth names what was added, removed and changed by field',()=>{
  const before={links:[link('plain','I.01'),link('gone','I.02'),link('built','I.02')],concepts:[{id:'c',title:{ru:'Понятие'}}]};
  const after={links:[link('plain','I.01'),link('built','I.02',{verb:{ru:'строит',en:'builds'},status:'reconstruction'}),link('new','I.03')],concepts:[{id:'c',title:{ru:'Понятие'}}]};
  const growth=growthBetween(before,after);
  const row=growth.collections.find(item=>item.key==='links');
  expect(row.added).toEqual(['new']);expect(row.removed).toEqual(['gone']);
  expect(row.changed).toEqual([{id:'built',fields:['status','verb']}]);
  expect(growth.collections.find(item=>item.key==='concepts').changed).toEqual([]);
  expect(growth.total).toBe(3);expect([...growth.changedIds].sort()).toEqual(['built','new']);
});

test('the stages of a subject follow its record: maker, former wordings with reviewers, review, admission',()=>{
  const revised=links.find(item=>item.verb_revision_history?.length);
  const stages=subjectStages(revised);
  expect(stages[0]).toEqual({kind:'made',who:revised.maker});
  const before=stages.find(stage=>stage.kind==='before');
  expect(before).toMatchObject({field:'verb',text:revised.verb_revision_history[0].verb});
  expect(before.who).toBeTruthy();
  expect(stages.some(stage=>stage.kind==='review'&&stage.verdict===revised.assessment.verdict)).toBe(true);
  expect(stages.at(-1)).toEqual({kind:'admission',state:revised.admission});
  expect(subjectStages(proposed[0]).some(stage=>stage.kind==='review'&&stage.model)).toBe(true);
});

test('a visit remembers the revision it saw; the different one before stays the last visit',()=>{
  const data=new Map(),storage={getItem:key=>data.get(key)??null,setItem:(key,value)=>data.set(key,value)};
  const a='a'.repeat(64),b='b'.repeat(64);
  expect(rememberRevision(storage,'zarathustra',a)).toEqual({current:a,previous:null});
  expect(rememberRevision(storage,'zarathustra',a)).toEqual({current:a,previous:null});
  expect(rememberRevision(storage,'zarathustra',b)).toEqual({current:b,previous:a});
  expect(rememberRevision(storage,'zarathustra',b)).toEqual({current:b,previous:a});
  expect(JSON.parse(data.get(SEEN_REVISION_KEY)).zarathustra).toEqual({current:b,previous:a});
  expect(rememberRevision(storage,'zarathustra','not-a-revision')).toEqual({current:'not-a-revision',previous:null});
});

function setup({stored=null}={}){
  const window=new Window({url:'http://localhost/zarathustra/'});
  const document=window.document;document.body.innerHTML='<div id="tree"></div>';
  const shell=document.querySelector('#tree');
  const current='c'.repeat(64),old='0'.repeat(64);
  const items=new Map([...core.links,...core.proposed].map(record=>[record.id,{id:record.id,kind:record.from?'relation':'conception',title:record.verb??record.title,record}]));
  items.set('a',{id:'a',title:{ru:'Да',en:'Yes'}});items.set('b',{id:'b',title:{ru:'Дитя',en:'Child'}});
  const blocks=[];
  const space={index:{revision:current,get:id=>items.get(id)??null},actions:{core:()=>core,addMeasureBlock:render=>{blocks.push(render);return ()=>{};},showChapter:vi.fn(async()=>{})}};
  const acts=createActJournal({store:createMemoryActStore(),locks:null,channel:null});
  const added=[];const actionLayer={addActions:list=>{added.push(...list);return ()=>{};},say:vi.fn()};
  const data=new Map(stored?[[SEEN_REVISION_KEY,JSON.stringify({zarathustra:{current:old}})]]:[]);
  const storage={getItem:key=>data.get(key)??null,setItem:(key,value)=>data.set(key,value)};
  const oldCore={...core,links:core.links.map(item=>item.id==='argued'?{...item,verb:{ru:'вело к',en:'led to'}}:item).filter(item=>item.id!=='plain')};
  const versions={list:()=>[current,old],load:vi.fn(async revision=>revision===old?{status:'available',core:oldCore}:{status:'version-unavailable'})};
  const actDesk={openTicket:vi.fn(async()=>{})};
  const experience={open:vi.fn(),setMode:vi.fn()};
  const trail=mountResearchTrail({shell,actionLayer,space,acts,actDesk,experience,locale:()=> 'en',storage,versions});
  return {window,document,acts,trail,added,actionLayer,blocks,actDesk,experience,space,old};
}

test('the trail adds its actions and its block in the measure panel',()=>{
  const {added,blocks,document}=setup();
  expect(added.map(action=>action.id)).toEqual(['frontier','history','growth']);
  const panel=document.createElement('div'),bars=vi.fn();blocks[0](panel,{bars});
  expect(bars.mock.calls[0][1]).toBe('Frontier');
  expect(bars.mock.calls[0][2].map(row=>[row.label,row.n])).toEqual([['Chapters without links of thought',witnesses.chapters.length-3],['Links resting on a reconstruction',1],['Disputes left open',1],['Rejected by review',1],['Proposed by the stand',1]]);
});

test('the frontier opens its places, marks the person\'s tickets and files a ticket with its category',async()=>{
  const {document,acts,trail,actDesk,space}=setup();
  await acts.act({kind:'ticket',targets:['built'],grounds:[],intent:'find a sentence',body:{category:'relation',observation:'find a sentence'}});
  await trail.openFrontier('reconstructions');
  await vi.waitFor(()=>expect(document.querySelector('.rt-entry')).not.toBeNull());
  expect(document.querySelector('.rt-entry .rt-title').textContent).toBe('Yes — leads to — Child');
  expect(document.querySelector('.rt-mark').textContent).toBe('your ticket · Open');
  const buttons=[...document.querySelectorAll('.rt-entry button')];
  buttons.find(node=>node.textContent==='Ticket').click();
  expect(actDesk.openTicket).toHaveBeenCalledWith(expect.objectContaining({type:'link',id:'built'}),{category:'relation'});
  await trail.openFrontier('chapters');
  const sheet=()=>[...document.querySelectorAll('.rt-sheet')].at(-1);
  // The stand's own chapters: I.01, I.02 and II.01 carry links, so I.03 comes first among the open ones.
  await vi.waitFor(()=>expect(sheet().querySelector('.rt-entry .rt-place')?.textContent).toBe('I.03'));
  [...sheet().querySelectorAll('.rt-entry button')].find(node=>node.textContent==='Read').click();
  expect(space.actions.showChapter).toHaveBeenCalledWith('I.03');
  await trail.openFrontier('chapters');
  [...sheet().querySelectorAll('.rt-entry button')].find(node=>node.textContent==='Ticket').click();
  expect(actDesk.openTicket).toHaveBeenLastCalledWith(expect.objectContaining({type:'passage',stand:{language:'de',chapter:'I.03'}}),{category:'reading'});
});

test('what grew since the last visit compares the version that visit saw with the current core',async()=>{
  const {document,trail,actionLayer,acts,old,experience}=setup({stored:true});
  expect(trail.seen().previous).toBe(old);
  expect(actionLayer.say).toHaveBeenCalledWith('The stand\'s core changed since your last visit');
  await acts.act({kind:'ticket',targets:['argued'],grounds:[],intent:'which verb',body:{category:'relation',observation:'which verb'}});
  await trail.openGrowth();
  await vi.waitFor(()=>expect(document.querySelector('.rt-collection')).not.toBeNull());
  expect(document.querySelector('.ad-head h2').textContent).toBe('What grew since the last visit');
  expect(document.querySelector('.rt-collection h3').textContent).toBe('Links · added 1 · changed 1');
  expect(document.querySelector('.rt-touched').textContent).toBe('1 touch your tickets');
  const first=document.querySelector('.rt-collection .rt-entry');
  expect(first.querySelector('.rt-mark')).not.toBeNull();
  [...first.querySelectorAll('button')].find(node=>node.textContent==='As it was').click();
  expect(experience.open).toHaveBeenCalledWith('argued',{revision:old});
});

test('the history of a subject shows its stages, the person\'s acts and its change against a version',async()=>{
  const {document,trail,acts,old}=setup();
  await acts.act({kind:'note',targets:['argued'],grounds:[],intent:'the lion first',body:{text:'the lion first'}});
  await trail.openHistory({type:'link',id:'argued',label:'Yes — leads to — Child'});
  await vi.waitFor(()=>expect(document.querySelector('.rt-acts li')).not.toBeNull());
  const stages=[...document.querySelectorAll('.rt-stage')].map(row=>row.dataset.kind);
  expect(stages).toEqual(['review','admission']);
  expect(document.querySelector('.rt-counter').textContent).toContain('иначе');
  expect(document.querySelector('.rt-acts li').textContent).toContain('the lion first');
  const picker=document.querySelector('.rt-version');picker.value=old;picker.dispatchEvent(new document.defaultView.Event('change'));
  await vi.waitFor(()=>expect(document.querySelector('.rt-diff')).not.toBeNull());
  expect(document.querySelector('.rt-diff strong').textContent).toBe('verb');
  expect(document.querySelector('.rt-then').textContent).toBe('then: led to');
  expect(document.querySelector('.rt-now').textContent).toBe('now: leads to');
});
