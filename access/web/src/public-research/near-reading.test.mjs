import {expect,test} from 'vitest';
import {nearCount,nearLevel,NEAR_ZOOM} from './near-reading.mjs';

test('the nearer the reader comes to a chosen star, the more of its text stands beside it',()=>{
  expect(nearLevel({zoom:1,selected:'s'})).toBe(0);
  expect(nearLevel({zoom:NEAR_ZOOM[0],selected:'s'})).toBe(1);
  expect(nearLevel({zoom:NEAR_ZOOM[1]+1,selected:'s'})).toBe(2);
  expect([0,1,2].map(nearCount)).toEqual([0,1,3]);
});

test('without a chosen star, on a route, in a measured lens or on the workbench nothing is shown',()=>{
  for(const view of [{zoom:9},{zoom:9,selected:'s',route:true},{zoom:9,selected:'s',measured:true},{zoom:9,selected:'s',bench:true},{zoom:NaN,selected:'s'}])expect(nearLevel(view)).toBe(0);
});
