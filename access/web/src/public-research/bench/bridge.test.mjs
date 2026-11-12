import {expect,test} from 'vitest';
import {benchListens,layOnBench,receiveOnBench} from './bridge.mjs';

test('a passage reaches the bench only while one is mounted to receive it',async()=>{
  expect(benchListens()).toBe(false);
  expect(await layOnBench({language:'ru',unitId:'u-1',start:0,end:4})).toBe(null);
  const laid=[];
  const stop=receiveOnBench(passage=>{laid.push(passage);return {id:'frag-1',label:'Когда'};});
  expect(benchListens()).toBe(true);
  expect(await layOnBench({language:'ru',unitId:'u-1',start:0,end:5})).toEqual({id:'frag-1',label:'Когда'});
  expect(laid).toEqual([{language:'ru',unitId:'u-1',start:0,end:5}]);
  stop();
  expect(benchListens()).toBe(false);
});

test('a bench that fails to take a passage answers with nothing, and a stale stop leaves the new bench listening',async()=>{
  const stopOld=receiveOnBench(()=>{throw new Error('chapter missing');});
  expect(await layOnBench({language:'de',unitId:'x',start:0,end:1})).toBe(null);
  const stopNew=receiveOnBench(async()=>({id:'frag-2',label:'Volk'}));
  stopOld();
  expect(benchListens()).toBe(true);
  expect(await layOnBench({cite:{w:'also-sprach-zarathustra',c:'I.01',u:['a','a'],s:0,e:4}})).toEqual({id:'frag-2',label:'Volk'});
  stopNew();
});
