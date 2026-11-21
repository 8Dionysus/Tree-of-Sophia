import '../observatory/human-forms-wasm-test-runtime.mjs';
import {expect,test,vi} from 'vitest';
import {Window} from 'happy-dom';
import {fieldItem,fieldOf,mountAgentField,routeForWalker} from './agent-field.mjs';
import {createActJournal} from '../act-journal/journal.mjs';
import {createMemoryActStore} from '../act-journal/store.mjs';
import {actJournalTools} from '../act-journal/agent-tools.mjs';

const agent={type:'agent',id:'cartographer-1',model:'model-a',role:'link-finder'};
const declare=(name,shape,required)=>({kind:'kind-declaration',author:agent,targets:[`kind:${name}`],grounds:[],intent:`declare ${name}`,body:{declares:{name,description:`${shape} of the cartographer`,grounds:'optional',parents:'optional',body_required:required,shape}}});

async function journalWithField(){
  const acts=createActJournal({store:createMemoryActStore(),locks:null,channel:null});
  const tools=Object.fromEntries(actJournalTools(acts,{tier:()=>'propose'}).map(tool=>[tool.name,tool]));
  const append=async input=>JSON.parse((await tools['tos.act.append'].execute(input)).content[0].text);
  await append(declare('x.cartographer.thought','thought',['title','text']));
  await append(declare('x.cartographer.lens','lens',['title','members']));
  await append(declare('x.cartographer.route','route',['title','steps']));
  await append({kind:'x.cartographer.thought',author:agent,targets:['child'],grounds:[],intent:'the child as a wheel',body:{title:'The child as a wheel',text:'A self-propelled wheel is the child\'s first movement.'}});
  await append({kind:'x.cartographer.lens',author:agent,targets:['child','lion'],grounds:[],intent:'a lens of beginnings',body:{title:'Beginnings',members:['child','lion'],groups:[{title:'Who begins',members:['child']}]}});
  await append({kind:'x.cartographer.route',author:agent,targets:['lion','child'],grounds:[],intent:'from the lion to the child',body:{title:'From no to yes',question:'How does the no become a yes?',steps:[{target:'lion',title:'The lion'},{target:'child',text:'The sacred yes.'}]}});
  // A kind nobody declared needs its grounds, as the journal's invariants say.
  await append({kind:'x.cartographer.whisper',author:agent,targets:['child'],grounds:['child'],intent:'an undeclared whisper',body:{}});
  return {acts,append};
}
const all=async acts=>(await acts.recent({limit:500})).map(row=>row.record);

test('the field reads what agents declared and built: shaped items, a kind without a declaration, nothing malformed',async()=>{
  const {acts}=await journalWithField();
  const field=fieldOf(await all(acts));
  expect(field.kinds.map(kind=>[kind.name,kind.shape,kind.acts])).toEqual(expect.arrayContaining([['x.cartographer.thought','thought',1],['x.cartographer.lens','lens',1],['x.cartographer.route','route',1]]));
  expect(field.items.map(item=>item.shape).sort()).toEqual(['lens','route','thought']);
  expect(field.undeclared).toEqual(['x.cartographer.whisper']);
  expect(fieldItem({id:'a',body:{title:'t',steps:[{target:'only-one'}]}},'route')).toBeNull();
  expect(fieldItem({id:'a',body:{from:'x',to:'x',verb:'is'}},'link')).toBeNull();
  const route=routeForWalker(field.items.find(item=>item.shape==='route'),{nameOf:id=>id.toUpperCase()});
  expect(route).toMatchObject({title:{en:'From no to yes'},steps:[{nodeId:'lion',title:{en:'The lion'}},{nodeId:'child',title:{en:'CHILD'},body:{en:'The sacred yes.'}}]});
});

test('a withdrawn act leaves the field',async()=>{
  const {acts,append}=await journalWithField();
  const thought=fieldOf(await all(acts)).items.find(item=>item.shape==='thought');
  await append({kind:'withdrawal',author:agent,targets:[`act:${thought.id}`],grounds:[],intent:'withdrawn',body:{}});
  expect(fieldOf(await all(acts)).items.some(item=>item.shape==='thought')).toBe(false);
});

test('the sheet shows the field; a lens is tried on, a route walked, an item goes to a ticket only by the person\'s act',async()=>{
  const {acts}=await journalWithField();
  const window=new Window({url:'http://localhost/zarathustra/'}),document=window.document;document.body.innerHTML='<div id="tree"></div>';
  const space={index:{get:id=>({child:{title:{en:'The child'}},lion:{title:{en:'The lion'}}})[id]??null},actions:{setTrialLens:vi.fn(),setAgentRoutes:vi.fn(),startRoute:vi.fn()}};
  const actDesk={openTicket:vi.fn(async()=>{})};
  const fieldView=mountAgentField({shell:document.querySelector('#tree'),actionLayer:{addActions:()=>()=>{}},acts,actDesk,space,locale:()=> 'en'});
  await fieldView.open();
  expect([...document.querySelectorAll('.af-kind code')].map(node=>node.textContent)).toContain('x.cartographer.whisper');
  expect(space.actions.setAgentRoutes).toHaveBeenCalledWith([expect.objectContaining({title:{ru:'From no to yes',en:'From no to yes'}})]);
  const lens=document.querySelector('.af-item[data-shape=lens]');
  expect(lens.textContent).toContain('Who begins: The child');
  [...lens.querySelectorAll('button')].find(node=>node.textContent==='Try on').click();
  expect(space.actions.setTrialLens).toHaveBeenCalledWith(expect.objectContaining({title:'Beginnings',members:['child','lion']}));
  await fieldView.open();
  [...document.querySelector('.af-item[data-shape=route]').querySelectorAll('button')].find(node=>node.textContent==='Walk').click();
  expect(space.actions.startRoute.mock.calls[0][0]).toMatch(/^agent:/);
  expect(actDesk.openTicket).not.toHaveBeenCalled();
  await fieldView.open();
  [...document.querySelector('.af-item[data-shape=thought]').querySelectorAll('button')].find(node=>node.textContent==='To a ticket').click();
  expect(actDesk.openTicket).toHaveBeenCalledWith(expect.objectContaining({type:'star',id:'child'}),expect.objectContaining({parents:[expect.any(String)],observation:expect.stringContaining('The child as a wheel')}));
});
