import {expect,test,vi} from 'vitest';
import {Window} from 'happy-dom';
import {applyWorkplace,captureWorkplace,createWorkplaceStore,mountWorkplaces,normalizeWorkplace,WORKPLACES_KEY} from './workplaces.mjs';

function page(){
  let mode='original',lens='tree',layer='reading',route=null,selection={id:'child',kind:'node'},benchOpen=false,active='bench-a';
  const calls=[];
  const actionLayer={context:()=>({presence_layer:layer,lens,route,bench_open:benchOpen}),layer:()=>layer,setLayer:value=>{layer=value;calls.push(['layer',value]);},say:vi.fn(),addActions:list=>{actionLayer.added=list;return ()=>{};}};
  const benchView={isOpen:()=>benchOpen,hide:()=>{benchOpen=false;calls.push(['hide']);},store:{get state(){return {active,benches:[{id:'bench-a'},{id:'bench-b'}]};},update:async op=>{active=op({active,benches:[{id:'bench-a'},{id:'bench-b'}]}).active;}}};
  const space={selection:()=>selection,bench:()=>benchView,openBench:()=>{benchOpen=true;calls.push(['bench']);},index:{get:id=>({title:{en:id}})},
    actions:{lens:()=>lens,setLens:value=>{lens=value;calls.push(['lens',value]);},lenses:()=>[{id:'tree',title:{en:'Tree'}},{id:'time',title:{en:'Time'}}],routes:()=>[{id:'r1',title:{en:'Route one'}}],
      startRoute:(id,step)=>{route={id,step};calls.push(['route',id,step]);return true;}}};
  const experience={getState:()=>({mode}),setMode:value=>{mode=value;calls.push(['mode',value]);},open:vi.fn()};
  const registry={invoke:vi.fn(async(name,input)=>{calls.push([name,input.item_id]);selection={id:input.item_id,kind:'node'};})};
  return {actionLayer,space,experience,registry,calls,set:{lens:v=>{lens=v;},layer:v=>{layer=v;},mode:v=>{mode=v;},route:v=>{route=v;},bench:(open,id)=>{benchOpen=open;active=id;},selection:v=>{selection=v;}}};
}

test('a workplace keeps the mode, lens, layer, route or subject and the workbench, and returns them in order',async()=>{
  const a=page();a.set.mode('reference');a.set.lens('time');a.set.layer('research');a.set.bench(true,'bench-b');
  const kept=captureWorkplace({...a,name:'Lion'});
  expect(kept).toMatchObject({name:'Lion',mode:'reference',lens:'time',layer:'research',route:null,selection:{id:'child',kind:'node'},bench:{open:true,active:'bench-b'}});
  const b=page();await applyWorkplace(kept,b);
  expect(b.calls).toEqual([['mode','reference'],['lens','time'],['layer','research'],['tos.page.select','child'],['bench']]);
  expect(b.space.bench().store.state.active).toBe('bench-b');
  // A route takes the place of a chosen subject and keeps its step.
  const c=page();c.set.route({id:'r1',source:'atlas',step:3});const onRoute=captureWorkplace(c);
  expect(onRoute.selection).toBeNull();const d=page();await applyWorkplace(onRoute,d);
  expect(d.calls).toContainEqual(['route','r1',3]);
});

test('a kept workplace is checked part by part',()=>{
  expect(normalizeWorkplace({mode:'nonsense',layer:'x',lens:'tree',route:{id:'r',step:-2},selection:{id:'',kind:'node'},bench:{open:'yes'}}))
    .toEqual({name:'',at:'',mode:'original',lens:'tree',layer:'reading',route:{id:'r',step:1},selection:null,bench:{open:false,active:null}});
  expect(normalizeWorkplace(null)).toBeNull();
  const data=new Map(),storage={getItem:key=>data.get(key)??null,setItem:(key,value)=>data.set(key,value)};
  data.set(`${WORKPLACES_KEY}.zarathustra`,'not json');
  expect(createWorkplaceStore({storage}).list()).toEqual(Array(9).fill(null));
});

test('Alt with a digit returns a place and Alt, Shift and the digit keep one; the list names each place',async()=>{
  const window=new Window({url:'http://localhost/zarathustra/'}),document=window.document;document.body.innerHTML='<div id="tree"></div>';
  const data=new Map(),storage={getItem:key=>data.get(key)??null,setItem:(key,value)=>data.set(key,value)};
  const a=page();a.set.lens('time');
  const places=mountWorkplaces({shell:document.querySelector('#tree'),...a,locale:()=> 'en',storage});
  const press=(code,shift=false)=>window.dispatchEvent(new window.KeyboardEvent('keydown',{code,altKey:true,shiftKey:shift,bubbles:true}));
  press('Digit2',true);
  expect(places.store.get(2)).toMatchObject({lens:'time'});
  a.set.lens('tree');a.calls.length=0;press('Digit2');
  await vi.waitFor(()=>expect(a.calls).toContainEqual(['lens','time']));
  await vi.waitFor(()=>expect(a.actionLayer.say).toHaveBeenLastCalledWith('Workplace returned: Place 2'));
  expect(a.actionLayer.added.filter(action=>action.when?.()??true).map(action=>action.id)).toEqual(['workplaces','workplace-2']);
  press('Digit0');
  const rows=[...document.querySelectorAll('.wp-row')];
  expect(rows).toHaveLength(9);
  expect(rows[1].textContent).toContain('Original · Time · Reading · child');
  places.destroy();
});
