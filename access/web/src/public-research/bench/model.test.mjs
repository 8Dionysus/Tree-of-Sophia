import {expect,test} from 'vitest';
import {activeBench,addBench,applyPreset,areas,cleanSubject,closeWindow,emptyState,findArea,moveWindow,openSubject,putFragment,putThought,
  releaseSizes,removeBench,setCollapsed,setFocus,setSizes,subjectKey,trayAdd,trayRemove,updateFragment,validateState,MAX_DEPTH} from './model.mjs';

const node=id=>({kind:'node',id});
const depth=(layout,d=0)=>!layout||layout.area?d:Math.max(...layout.children.map(child=>depth(child,d+1)));

test('the first window fills the bench; the same subject comes forward instead of opening twice',()=>{
  let {state,windowId}=openSubject(emptyState(),node('material:overman'));
  const bench=activeBench(state);
  expect(bench.layout).toEqual({area:expect.any(String),tabs:[windowId],active:windowId,collapsed:false});
  const again=openSubject(state,node('material:overman'));
  expect(again.windowId).toBe(windowId);expect(Object.keys(activeBench(again.state).windows)).toHaveLength(1);
});

test('a window of another family opens beside the work; one of the same family joins its area as a tab',()=>{
  let s=emptyState(),a,b,c;
  ({state:s,windowId:a}=openSubject(s,{kind:'chapter',chapter:'I.22',language:'de'}));
  ({state:s,windowId:b}=openSubject(s,node('material:bestowing-virtue')));
  ({state:s,windowId:c}=openSubject(s,node('material:gold')));
  const bench=activeBench(s);
  expect(bench.layout.split).toBe('row');
  expect(areas(bench.layout).map(area=>area.tabs)).toEqual([[a],[b,c]]);
});

test('the word lies beside the text and the thought under the rest',()=>{
  let s=emptyState(),t,w,m;
  ({state:s,windowId:t}=openSubject(s,{kind:'chapter',chapter:'I.22',language:'de'}));
  ({state:s}=openSubject(s,node('material:bestowing-virtue')));
  ({state:s,windowId:w}=openSubject(s,{kind:'word',form:'schenken',language:'de'}));
  ({state:s,windowId:m}=openSubject(s,{kind:'thought',id:'thought-1'}));
  const bench=activeBench(s);
  expect(bench.layout.split).toBe('column');
  const row=bench.layout.children[0];
  expect(row.split).toBe('row');
  expect(row.children.map(area=>area.tabs[0])).toEqual([t,w,expect.any(String)]);
  expect(bench.layout.children[1].tabs).toEqual([m]);
  expect(depth(bench.layout)).toBeLessThanOrEqual(MAX_DEPTH);
});

test('moving a window to an edge splits the target; the tree never grows deeper than two levels',()=>{
  let s=emptyState();const ids=[];
  for(const id of ['a','b','c','d','e']){const r=openSubject(s,node(id),{place:'tab'});s=r.state;ids.push(r.windowId);}
  const first=areas(activeBench(s).layout)[0].area;
  s=moveWindow(s,ids[1],{area:first,edge:'right'});
  let target=areas(activeBench(s).layout).find(area=>area.tabs.includes(ids[1])).area;
  s=moveWindow(s,ids[2],{area:target,edge:'bottom'});
  target=areas(activeBench(s).layout).find(area=>area.tabs.includes(ids[2])).area;
  s=moveWindow(s,ids[3],{area:target,edge:'left'});
  target=areas(activeBench(s).layout).find(area=>area.tabs.includes(ids[3])).area;
  s=moveWindow(s,ids[4],{area:target,edge:'top'});
  const bench=activeBench(s);
  expect(depth(bench.layout)).toBeLessThanOrEqual(MAX_DEPTH);
  expect(areas(bench.layout).flatMap(area=>area.tabs).sort()).toEqual([...ids].sort());
});

test('closing the last window of an area takes the area away and keeps the rest sound',()=>{
  let s=emptyState(),a,b;
  ({state:s,windowId:a}=openSubject(s,{kind:'chapter',chapter:'I.01',language:'de'}));
  ({state:s,windowId:b}=openSubject(s,node('x')));
  s=closeWindow(s,b);
  const bench=activeBench(s);
  expect(bench.layout.area).toBeTruthy();expect(bench.layout.tabs).toEqual([a]);expect(bench.windows[b]).toBeUndefined();
});

test('a split set by hand keeps its shares until it is given back to breathing',()=>{
  let s=emptyState();
  s=openSubject(s,{kind:'chapter',chapter:'I.01',language:'de'}).state;s=openSubject(s,node('x')).state;
  s=setSizes(s,[],[3,1]);
  expect(activeBench(s).layout.sizes).toEqual([0.75,0.25]);expect(activeBench(s).layout.pinned).toBe(true);
  s=releaseSizes(s,[]);expect(activeBench(s).layout.pinned).toBe(false);
});

test('focus, folding, the tray and several benches',()=>{
  let s=emptyState(),a;
  ({state:s,windowId:a}=openSubject(s,node('x')));
  s=setFocus(s,a);expect(activeBench(s).focus).toBe(a);
  const area=areas(activeBench(s).layout)[0].area;s=setCollapsed(s,area,true);expect(areas(activeBench(s).layout)[0].collapsed).toBe(true);
  s=trayAdd(s,node('y'),{title:'Y'});s=trayAdd(s,node('y'),{title:'Y'});
  expect(activeBench(s).tray).toHaveLength(1);
  s=trayRemove(s,subjectKey(node('y')));expect(activeBench(s).tray).toHaveLength(0);
  s=addBench(s,{title:'Вечное возвращение'});expect(s.benches).toHaveLength(2);expect(activeBench(s).layout).toBeNull();
  s=removeBench(s,s.active);expect(s.benches).toHaveLength(1);
});

test('fragments keep their place once, and grow notes and links',()=>{
  const cite={w:'also-sprach-zarathustra',c:'I.22',u:['u1','u2'],s:3,e:40};
  let {state:s,id}=putFragment(emptyState(),{cite,witness:{language:'de'},label:'eine schenkende Tugend'});
  const again=putFragment(s,{cite,witness:{language:'de'},label:'other'});
  expect(again.id).toBe(id);
  s=updateFragment(s,id,f=>({notes:[...f.notes,{kind:'question',body:'Чем дар отличается от обмена?'}],links:[...f.links,{verb:'answer',target:{kind:'node',id:'material:x'}}]}));
  expect(s.fragments[id].notes[0]).toMatchObject({kind:'question',body:'Чем дар отличается от обмена?'});
  expect(s.fragments[id].links[0].target).toEqual({kind:'node',id:'material:x'});
  s=updateFragment(s,id,f=>({notes:[...f.notes,{kind:'shout',body:'x'}]}));
  expect(s.fragments[id].notes).toHaveLength(1);
});

test('the prepared layouts lay the bench out around what it holds',()=>{
  let s=emptyState();
  s=openSubject(s,node('material:overman')).state;
  s=applyPreset(s,'analysis',{chapter:'I.01'});
  const bench=activeBench(s),views=Object.values(bench.windows).map(win=>win.view).sort();
  expect(views).toEqual(['card','text','thought','word']);
  expect(bench.layout.split).toBe('column');
  const thought=Object.values(bench.windows).find(win=>win.view==='thought');
  expect(s.thoughts[thought.subject.id]).toBeTruthy();
  expect(applyPreset(s,'compare')).toBe(s);
});

test('a stored state is read soundly: broken windows and unknown fields leave',()=>{
  let s=openSubject(emptyState(),node('material:x')).state;
  s=putThought(s,{question:'Q',thesis:'T',grounds:['frag-1','frag-1']}).state;
  const raw=JSON.parse(JSON.stringify(s));
  raw.benches[0].windows.bad={view:'card',subject:{kind:'chapter',chapter:'I.01'}};
  raw.benches[0].layout={split:'row',sizes:[1,1],children:[raw.benches[0].layout,{area:'z',tabs:['bad','ghost']}]};
  raw.benches[0].extra='leaves';
  const read=validateState(raw);
  expect(Object.keys(read.benches[0].windows)).toHaveLength(1);
  expect(read.benches[0].layout.area).toBeTruthy();
  expect(read.benches[0].extra).toBeUndefined();
  expect(Object.values(read.thoughts)[0].grounds).toEqual(['frag-1']);
  expect(()=>validateState({schema:'other'})).toThrow();
});

test('an area is found by its path',()=>{
  let s=emptyState(),w;
  s=openSubject(s,{kind:'chapter',chapter:'I.01',language:'de'}).state;({state:s,windowId:w}=openSubject(s,node('x')));
  const bench=activeBench(s),area=areas(bench.layout).find(item=>item.tabs.includes(w));
  expect(findArea(bench.layout,area.area)).toEqual([1]);
});

test('a word keeps the place it was read at, so the philological engine finds that very word',()=>{
  expect(cleanSubject({kind:'word',form:'Übermenschen',language:'de',chapter:'I.01',unit:'u-12',at:41})).toEqual({kind:'word',form:'Übermenschen',language:'de',chapter:'I.01',unit:'u-12',at:41});
  expect(cleanSubject({kind:'word',form:'Volk',at:-3})).toEqual({kind:'word',form:'Volk',language:'de'});
  expect(cleanSubject({kind:'word',form:'Volk',at:'7'})).toEqual({kind:'word',form:'Volk',language:'de'});
});
