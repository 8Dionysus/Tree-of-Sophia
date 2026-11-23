import '../observatory/human-forms-wasm-test-runtime.mjs';
import {expect,test,vi} from 'vitest';
import {Window} from 'happy-dom';
import {createLocalSurface,mountSurface,openSurface,surfaceContract} from './surface.mjs';
import {createActJournal} from '../act-journal/journal.mjs';
import {createMemoryActStore} from '../act-journal/store.mjs';
import {draftAct} from '../act-journal/drafts.mjs';

const items=new Map([
  ['zarathustra',{id:'zarathustra',kind:'figure',title:{ru:'Заратустра',en:'Zarathustra'},dependencies:[]}],
  ['eternal-return',{id:'eternal-return',kind:'concept',title:{ru:'Вечное возвращение',en:'Eternal return'},dependencies:['zarathustra']}],
]);

function setup({tier='propose',storage=null}={}){
  const window=new Window({url:'http://localhost/zarathustra/'});
  const document=window.document;document.body.innerHTML='<div id="tree"><div class="panel"></div></div>';
  globalThis.performance??=window.performance;
  const shell=document.querySelector('#tree');
  let card=null,sky=null,selected=null;
  const space={
    index:{get:id=>items.get(id)??null,search:query=>[...items.values()].filter(item=>item.title.en.toLowerCase().includes(String(query).toLowerCase())),
      inspect:id=>{const item=items.get(id);return item?{...item,dependencies:item.dependencies.map(dep=>({id:dep,kind:items.get(dep)?.kind})),referencedBy:[...items.values()].filter(other=>other.dependencies.includes(id))}:null;}},
    selection:()=>selected?{id:selected}:null,
    actions:{setSurfaceSky:(spec,options)=>{sky=spec?{...spec,off:options?.off}:null;return Boolean(spec);},addCardBlock:render=>{card=render;return ()=>{card=null;};},refreshCard:()=>{}},
  };
  const acts=createActJournal({store:createMemoryActStore(),locks:null,channel:null});
  const invoked=[];const registry={invoke:vi.fn(async(command,input)=>{invoked.push([command,input]);return {};})};
  const store=new Map();const session=storage??{getItem:key=>store.get(key)??null,setItem:(key,value)=>store.set(key,value)};
  const surface=createLocalSurface({storage:session,key:'k'});
  const view=mountSurface({shell,surface,acts,space,registry:()=>registry,viewState:()=>({presence_layer:'reading',lens:'tree'}),actTier:()=>tier,locale:()=> 'en'});
  const tool=async(name,input={})=>{const found=view.tools.find(item=>item.name===name);const out=await found.execute(input);return {error:Boolean(out.isError),value:JSON.parse(out.content[0].text)};};
  return {window,document,shell,acts,view,tool,surface,invoked,session,select:id=>{selected=id;},card:()=>card,sky:()=>sky};
}

test('an agent reads the contract first and shapes the dock in one patch, answered with what was drawn',async()=>{
  const {document,tool}=setup();
  const {value:described}=await tool('tos.surface.describe');
  expect(described.contract.places.map(place=>place.name)).toEqual(['dock','strip','card','sky']);
  expect(described.state.revision).toBe(0);
  expect(described.may_patch).toBe(true);
  const {error,value}=await tool('tos.surface.patch',{author:{id:'a1',model:'m1'},ops:[
    {op:'title',text:'What I watch'},
    {op:'put',block:{id:'h',kind:'heading',text:'Subjects'}},
    {op:'put',block:{id:'found',kind:'list',title:'{label}',meta:'{kind} · {nothing}',target:'{id}',bind:{source:'search',query:'r'}}},
    {op:'put',in:'strip',block:{id:'n',kind:'metric',label:'subjects',bind:{source:'search',query:'r'}}},
  ]});
  expect(error).toBe(false);
  expect(value.revision).toBe(1);
  expect(value.changed).toEqual(['found','h','n']);
  const found=value.drawn.blocks.find(line=>line.id==='found');
  expect(found).toMatchObject({place:'dock',count:2,total:2});
  expect(found.notes[0]).toContain('no row has nothing');
  expect(document.querySelector('.sf-dock').hidden).toBe(false);
  expect(document.querySelector('.sf-title').textContent).toBe('What I watch');
  expect(document.querySelector('.sf-by').textContent).toContain('a1 · m1');
  expect([...document.querySelectorAll('.sf-list .sf-item-title')].map(node=>node.textContent)).toEqual(['Zarathustra','Eternal return']);
  expect(document.querySelector('.sf-strip .sf-value').textContent).toBe('2');
  expect(document.querySelector('#tree').dataset.surfaceStrip).toBe('');
});

test('a refused patch changes nothing and says which op and what to write instead',async()=>{
  const {tool,document}=setup();
  await tool('tos.surface.patch',{ops:[{op:'put',block:{id:'t',kind:'text',text:'one'}}]});
  const {error,value}=await tool('tos.surface.patch',{ops:[{op:'set',id:'t',field:'text',value:'two'},{op:'put',block:{id:'x',kind:'text',text:'<b>x</b>',html:true}}]});
  expect(error).toBe(true);
  expect(value).toMatchObject({error:'surface_field',op:1});
  expect(value.message).toContain('its fields');
  expect(document.querySelector('.sf-text p').textContent).toBe('one');
});

test('a list bound to the journal follows the acts as they arrive',async()=>{
  const {tool,acts,document,view}=setup();
  await tool('tos.surface.patch',{ops:[{op:'put',block:{id:'kinds',kind:'bars',label:'{kind}',value:'count',bind:{source:'journal',view:'summary',path:'by_kind'}}}]});
  const before=document.querySelectorAll('.sf-bar').length;
  await acts.act({kind:'note',targets:['zarathustra'],intent:'a',body:{text:'a'}});
  await new Promise(resolve=>setTimeout(resolve,400));
  await view.draw();
  expect(document.querySelectorAll('.sf-bar').length).toBeGreaterThanOrEqual(before);
  const {value}=await tool('tos.surface.try',{bind:{source:'journal',view:'summary'}});
  expect(value.fields.length).toBeGreaterThan(0);
  expect(value.count).toBe(1);
});

test('rows click through to the page, and a button runs its command only when pressed',async()=>{
  const {tool,document,invoked}=setup();
  await tool('tos.surface.patch',{ops:[
    {op:'put',block:{id:'c',kind:'chips',members:['zarathustra']}},
    {op:'put',block:{id:'b',kind:'button',label:'Meaning',command:'tos.page.set-lens',input:{lens:'meaning'}}},
  ]});
  expect(invoked).toEqual([]);
  document.querySelector('.sf-chip').click();
  document.querySelector('.sf-run').click();
  await new Promise(resolve=>setTimeout(resolve,0));
  expect(invoked).toEqual([['tos.page.select',{item_id:'zarathustra'}],['tos.page.set-lens',{lens:'meaning'}]]);
  expect(document.querySelector('.sf-chip').textContent).toBe('Zarathustra');
});

test('the sky takes the agent\'s light, and the person takes it off',async()=>{
  const {tool,sky,surface}=setup();
  await tool('tos.surface.patch',{ops:[{op:'put',in:'sky',block:{id:'s',kind:'highlight',title:'Return',members:['eternal-return'],dim_rest:true}}]});
  expect(sky()).toMatchObject({members:['eternal-return'],dimRest:true});
  expect(sky().title).toContain('Return');
  sky().off();
  await new Promise(resolve=>setTimeout(resolve,10));
  expect((await surface.state()).places.sky).toEqual([]);
});

test('the card of the selected star reads it as $selected',async()=>{
  const {tool,card,select,document,view}=setup();
  await tool('tos.surface.patch',{ops:[{op:'put',in:'card',block:{id:'deps',kind:'list',title:'{label}',target:'{id}',bind:{source:'item',id:'$selected',path:'dependencies'}}}]});
  select('eternal-return');
  const host=document.querySelector('.panel');card()(host,'eternal-return');
  await view.draw();
  expect(host.querySelector('.sf-card .sf-item-title')?.textContent).toBe('Zarathustra');
});

test('the person freezes, undoes and clears; at the read tier agents only read',async()=>{
  const {tool,document}=setup();
  await tool('tos.surface.patch',{ops:[{op:'put',block:{id:'t',kind:'text',text:'one'}}]});
  await tool('tos.surface.patch',{ops:[{op:'set',id:'t',field:'text',value:'two'}]});
  document.querySelector('.sf-freeze').click();
  await new Promise(resolve=>setTimeout(resolve,10));
  expect((await tool('tos.surface.patch',{ops:[{op:'remove',id:'t'}]})).value.error).toBe('surface_frozen');
  document.querySelector('.sf-freeze').click();
  await new Promise(resolve=>setTimeout(resolve,10));
  document.querySelector('.sf-undo').click();
  await new Promise(resolve=>setTimeout(resolve,10));
  expect(document.querySelector('.sf-text p').textContent).toBe('one');
  document.querySelector('.sf-clear').click();
  await new Promise(resolve=>setTimeout(resolve,10));
  expect(document.querySelector('.sf-dock').hidden).toBe(true);
  const reader=setup({tier:'read'});
  expect((await reader.tool('tos.surface.patch',{ops:[{op:'title',text:'x'}]})).value.error).toBe('surface_tier');
  expect((await reader.tool('tos.surface.describe')).value.may_patch).toBe(false);
});

test('a kept surface is an act of the journal that opens again, and a reload keeps the clay',async()=>{
  const {tool,acts,session}=setup();
  await tool('tos.surface.patch',{ops:[{op:'title',text:'Review desk'},{op:'put',block:{id:'t',kind:'text',text:'kept'}}]});
  expect((await tool('tos.surface.keep',{author:{id:'a1'}})).value.error).toBe('act_invalid:author.model');
  const {value:kept}=await tool('tos.surface.keep',{author:{id:'a1',model:'m1'}});
  const record=await acts.record(kept.kept);
  expect(record).toMatchObject({kind:'surface',targets:['surface:review-desk'],body:{title:'Review desk'}});
  const {value:again}=await tool('tos.surface.keep',{author:{id:'a1',model:'m1'}});
  expect((await acts.record(again.kept)).replaces).toBe(kept.kept);
  await tool('tos.surface.patch',{ops:[{op:'clear'}]});
  const {value:opened}=await tool('tos.surface.open',{act:kept.kept});
  expect(opened.state.places.dock[0].text).toBe('kept');
  const reloaded=createLocalSurface({storage:session,key:'k'});
  expect((await reloaded.state()).places.dock[0].text).toBe('kept');
});

test('the page chooses the bridge\'s surface when the journal is the bridge\'s',async()=>{
  const state={schema:'tos.surface.v1',revision:4,title:'At the bridge',frozen:false,authors:[{id:'army-1'}],blocks:1,can_undo:0,places:{dock:[{id:'t',kind:'text',text:'from the bridge'}],strip:[],card:[],sky:[]}};
  const applied=[],receipts=[];let heard=null;
  const channel={state:async()=>state,apply:async body=>{applied.push(body);return {revision:5,changed:[],removed:[],state:{...state,revision:5}};},receipt:async value=>{receipts.push(value);},listen:fn=>{heard=fn;return ()=>{};}};
  const surface=openSurface({acts:{chosen:Promise.resolve({surfaceChannel:()=>channel})}});
  await surface.ready;
  expect(surface.tier).toBe('bridge');
  expect((await surface.state()).title).toBe('At the bridge');
  await surface.patch({author:{id:'a'},ops:[{op:'title',text:'x'}]});
  expect(applied[0].patch.ops[0].op).toBe('title');
  expect(typeof heard).toBe('function');
  expect(surfaceContract().schema).toBe('tos.surface.spec.v1');
  // The page's own draft act record stays checkable by the journal: a surface act is a known kind.
  expect(draftAct({kind:'surface',targets:['surface:x'],intent:'x',body:{title:'x',surface:{title:'x',places:{}}}}).kind).toBe('surface');
  expect(receipts).toEqual([]);
});
