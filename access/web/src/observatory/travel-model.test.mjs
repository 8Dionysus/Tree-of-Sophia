import {test,expect} from 'vitest';
import {capturePlace} from './place-model.mjs';
import {createTravelStore,createTravelNavigation,validateHistory,historyLabel,travelKey,compactHistory,HISTORY_KEY,HISTORY_LIMIT} from './travel-model.mjs';

const packet={schema:'tos_lens_result_v1',source_revision:'a'.repeat(64),authority_boundary:{is_source:false,is_canon:false,writes_to_tree:false},
  nodes:[{id:'opaque:a',source_refs:['ToS/example'],content_revision:'b'.repeat(64),display:{title:{ru:'Частный текст не входит в историю'}}}],relations:[],page:{next_cursor:'ephemeral-cursor'}};
function place(id,source=packet){return capturePlace(source,{lens:'constellations',yaw:Number(id)||0,pitch:0,zoom:1,pan:{x:0,y:0},selectedId:'opaque:'+id,relationId:null,panelOpen:true,cardTab:'about',vertices:[]},{name:'Шаг '+id,id:String(id),route:'?focus=opaque%3Aa',savedAt:1});}
function storage(){const values=new Map();return {getItem:key=>values.get(key)||null,setItem:(key,value)=>values.set(key,value)};}
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};};

test('back, forward and jumps preserve both sides; a new action branches at the cursor and survives reopening',()=>{
  const s=storage(),store=createTravelStore(s);for(let i=0;i<5;i++)store.record(place(i));
  store.move('1');expect(store.state.cursor).toBe(1);expect(store.state.entries).toHaveLength(5);
  store.move('4');store.move('2');store.save();const reopened=createTravelStore(s);expect(reopened.state.cursor).toBe(2);
  expect(reopened.resume('')).toEqual(place(2));expect(reopened.resume('?focus=other')).toBeNull();
  reopened.record(place(9));reopened.save();expect(createTravelStore(s).state.entries.map(e=>e.id)).toEqual(['0','1','2','9']);
});
test('history is bounded, strips packets and cursors, and does not count tool visibility as a step',()=>{
  const s=storage(),store=createTravelStore(s);for(let i=0;i<110;i++)store.record(place(i));
  expect(store.state.entries).toHaveLength(HISTORY_LIMIT);expect(store.state.entries[0].id).toBe('10');expect(store.state.cursor).toBe(99);
  const current=place(109);current.pose.panelOpen=false;expect(store.record(current)).toBe(false);store.save();
  const text=s.getItem(HISTORY_KEY);for(const excluded of ['ephemeral-cursor','source_refs','Частный текст','authority_boundary'])expect(text).not.toContain(excluded);
  expect(travelKey({...place(1),graph:packet})).toBe(travelKey(place(1)));
  expect(()=>place(1,{...packet,toJSON(){throw new Error('Do not serialize delivered source packets');}})).not.toThrow();
});
test('large valid requests also obey the total storage bound, trimming the oldest steps first',()=>{
  const s=storage(),store=createTravelStore(s);
  for(let i=0;i<40;i++)store.record({...place(i),route:'?query='+'x'.repeat(45000)});
  store.save();expect(s.getItem(HISTORY_KEY).length).toBeLessThanOrEqual(1200000);
  const state=createTravelStore(s).state;expect(state.entries.length).toBeLessThan(40);expect(state.entries.at(-1).id).toBe('39');expect(state.cursor).toBe(state.entries.length-1);
});
test('corrupt, oversized and future formats stay intact until an explicit clear',()=>{
  for(const raw of ['broken',JSON.stringify({v:2,entries:[],cursor:-1}),'x'.repeat(1200001)]){
    const s=storage();s.setItem(HISTORY_KEY,raw);const store=createTravelStore(s);expect(store.error).toBeTruthy();store.record(place(1));expect(store.save()).toBe(false);expect(s.getItem(HISTORY_KEY)).toBe(raw);
    store.clear(place(1));expect(createTravelStore(s).state.entries).toHaveLength(1);
  }
  for(const value of [{v:1,entries:[],cursor:0},{v:1,entries:[place(1)],cursor:-1},{v:1,entries:[place(1),place(1)],cursor:0},{v:1,entries:[{...place(1),savedAt:1e300}],cursor:0}])expect(()=>validateHistory(value)).toThrow();
});
test('storage failure leaves local navigation usable and retries; competing tabs cannot overwrite another history',()=>{
  const s=storage(),first=createTravelStore(s),second=createTravelStore(s);first.record(place(1));first.save();second.record(place(2));expect(second.save()).toBe(false);
  expect(second.error).toContain('другой вкладке');expect(createTravelStore(s).state.entries.map(e=>e.id)).toEqual(['1']);
  let fail=true;const quota={getItem:s.getItem,setItem:(...args)=>{if(fail)throw new Error('Нет места');s.setItem(...args);}};
  const store=createTravelStore(quota);store.record(place(3));expect(store.save()).toBe(false);expect(store.state.cursor).toBe(1);fail=false;expect(store.save()).toBe(true);
});
test('late success and late error cannot move the cursor or apply over a newer navigation',async()=>{
  const store=createTravelStore(storage());for(let i=0;i<3;i++)store.record(place(i));
  const calls=[],applied=[];const navigation=createTravelNavigation({store,reopen:entry=>{const reply=deferred();calls.push(reply);return reply.promise;},apply:value=>applied.push(value)});
  const old=navigation.go('0'),fresh=navigation.go('1');calls[1].resolve('new');expect((await fresh).current).toBe(true);calls[0].resolve('old');expect((await old).current).toBe(false);
  expect(applied).toEqual(['new']);expect(store.state.cursor).toBe(1);
  const cancelled=navigation.go('0');navigation.cancel();calls[2].reject(new Error('late failure'));expect((await cancelled).current).toBe(false);expect(store.state.cursor).toBe(1);
});
test('failed reopen keeps the current step; explicit retry can succeed',async()=>{
  const store=createTravelStore(storage());store.record(place(0));store.record(place(1));let fail=true,applied=0;
  const navigation=createTravelNavigation({store,reopen:async()=>{if(fail)throw new Error('Материал недоступен');return 'current';},apply:()=>applied++});
  await expect(navigation.go('0')).rejects.toThrow('недоступен');expect(store.state.cursor).toBe(1);expect(applied).toBe(0);
  fail=false;expect((await navigation.go('0')).current).toBe(true);expect(store.state.cursor).toBe(0);expect(applied).toBe(1);
});
test('step names identify materials and lenses; camera changes retain the current name',()=>{
  const first=place(1),next=structuredClone(first);next.pose.yaw=2;expect(historyLabel(first,next,'Имя')).toBe(first.name);
  next.pose.selectedId='opaque:b';expect(historyLabel(first,next,'Бета')).toBe('Бета');next.pose.selectedId=first.pose.selectedId;
  next.pose.lens='plane';expect(historyLabel(first,next,'Имя')).toBe('Линза: Карта связей');
  next.spec.seed.node_ids=['opaque:b'];expect(historyLabel(first,next,'Бета')).toBe('Бета');
  next.draft={changed:true};expect(historyLabel(first,next,'Имя')).toBe('Линза: Имя');
});

test('camera and card changes update the current stop without losing forward history or its identity',()=>{
  const s=storage(),store=createTravelStore(s);for(let i=0;i<3;i++)store.record(place(i));store.move('1');
  const changed={...place(1),id:'new-id',savedAt:800,pose:{...place(1).pose,yaw:8,zoom:2,cardTab:'relations'}};
  store.record(changed);store.save();const restored=createTravelStore(s).state;
  expect(restored.entries.map(e=>e.id)).toEqual(['0','1','2']);expect(restored.cursor).toBe(1);
  expect(restored.entries[1].pose.yaw).toBe(8);expect(restored.entries[1].pose.cardTab).toBe('relations');expect(restored.entries[1].savedAt).toBe(1);
});
test('legacy camera runs collapse around the cursor and keep other meaningful visits intact',()=>{
  const a=place(1),b=place(2),c=place(3),pan=(source,id,yaw)=>({...source,id,name:'Изменён ракурс',pose:{...source.pose,yaw}});
  const old={v:1,entries:[a,pan(a,'a2',4),b,pan(b,'b2',5),pan(b,'b3',6),c],cursor:3};
  const clean=compactHistory(old);expect(clean.entries.map(e=>e.id)).toEqual(['1','2','3']);expect(clean.cursor).toBe(1);
  expect(clean.entries[0].pose.yaw).toBe(4);expect(clean.entries[1].pose.yaw).toBe(5);expect(clean.entries[1].name).toBe(b.name);
  expect(compactHistory(clean)).toEqual(clean);
  const s=storage();s.setItem(HISTORY_KEY,JSON.stringify(old));createTravelStore(s);expect(JSON.parse(s.getItem(HISTORY_KEY))).toEqual(clean);
  expect(old.entries).toHaveLength(6);
});

test('updating a larger current pose keeps the total bound and never trims away the current stop',()=>{
  const entries=Array.from({length:100},(_,i)=>place(i)),old={v:1,entries,cursor:0};
  const padding=Math.floor((1200000-JSON.stringify(old).length-1000)/100);
  for(const entry of entries)entry.route+='x'.repeat(padding);
  const s=storage();s.setItem(HISTORY_KEY,JSON.stringify(old));const store=createTravelStore(s);
  store.record({...entries[0],id:'new-id',route:entries[0].route+'y'.repeat(5000),pose:{...entries[0].pose,zoom:2}});store.save();
  const result=createTravelStore(s).state;expect(result.cursor).toBe(0);expect(result.entries[0].id).toBe('0');expect(result.entries[0].pose.zoom).toBe(2);
  expect(s.getItem(HISTORY_KEY).length).toBeLessThanOrEqual(1200000);expect(result.entries.length).toBeLessThan(100);
});
