import {expect,test} from 'vitest';
import {activeBench,openSubject,trayAdd} from './model.mjs';
import {benchKey,createBenchStore} from './store.mjs';

const memory=()=>{const map=new Map();return {getItem:key=>map.get(key)??null,setItem:(key,value)=>map.set(key,String(value)),map};};

test('the store writes each change with a new revision and reads it back',()=>{
  const storage=memory(),store=createBenchStore({profile:'zarathustra',storage});
  store.update(state=>openSubject(state,{kind:'node',id:'material:x'}).state);
  expect(store.state.revision).toBe(1);
  const again=createBenchStore({profile:'zarathustra',storage});
  expect(Object.keys(activeBench(again.state).windows)).toHaveLength(1);
  store.dispose();again.dispose();
});

test('the way back and forward',()=>{
  const store=createBenchStore({profile:'zarathustra',storage:memory()});
  store.update(state=>trayAdd(state,{kind:'node',id:'a'}));
  store.update(state=>trayAdd(state,{kind:'node',id:'b'}));
  store.undo();expect(activeBench(store.state).tray).toHaveLength(1);
  store.redo();expect(activeBench(store.state).tray).toHaveLength(2);
  store.dispose();
});

test('a change made in another tab first is kept: the operation runs again on it',()=>{
  const storage=memory(),one=createBenchStore({profile:'zarathustra',storage}),two=createBenchStore({profile:'zarathustra',storage});
  one.update(state=>trayAdd(state,{kind:'node',id:'a'}));
  two.update(state=>trayAdd(state,{kind:'node',id:'b'}));
  const stored=JSON.parse(storage.map.get(benchKey('zarathustra')));
  expect(stored.benches[0].tray.map(item=>item.subject.id)).toEqual(['a','b']);
  one.dispose();two.dispose();
});

test('an unreadable stored bench gives way to an empty one and says why',()=>{
  const storage=memory();storage.setItem(benchKey('antiquity'),'{"schema":"other"}');
  const store=createBenchStore({profile:'antiquity',storage});
  expect(store.state.benches).toHaveLength(1);expect(store.error).toBeTruthy();
  store.dispose();
});
