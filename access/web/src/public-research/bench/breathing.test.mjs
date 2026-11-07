import {expect,test} from 'vitest';
import {combine,distribute,distributeColumn,measureOf,pinnedSizes,GUTTER,RAIL} from './breathing.mjs';

const text={...measureOf('text',{ch:8}),touched:1},card={...measureOf('card',{ch:8}),touched:2},word={...measureOf('word',{ch:8}),touched:3};
const sum=list=>list.reduce((a,b)=>a+b,0);

test('a view measures in characters of its own text',()=>{
  expect(measureOf('text',{ch:8})).toEqual({min:360,comfort:600,max:680});
  expect(measureOf('text',{ch:8,parallel:true}).comfort).toBeGreaterThan(measureOf('text',{ch:8}).comfort);
});

test('the window in which the work goes on gets its good measure first',()=>{
  const total=1200;
  const {sizes}=distribute(total,[{...text},{...card,active:true},{...word}]);
  expect(sum(sizes)+2*GUTTER).toBeLessThanOrEqual(total);
  expect(sizes[1]).toBeGreaterThanOrEqual(card.comfort-1);
  expect(sizes[0]).toBeGreaterThanOrEqual(text.min);expect(sizes[2]).toBeGreaterThanOrEqual(word.min);
});

test('a wide bench gives every window its good measure and shares the rest',()=>{
  const total=2400,{sizes}=distribute(total,[{...text,active:true},{...card},{...word}]);
  expect(sizes[0]).toBeGreaterThanOrEqual(text.comfort);expect(sizes[1]).toBeGreaterThanOrEqual(card.comfort);
  expect(sum(sizes)+2*GUTTER).toBeGreaterThan(total-4);
});

test('a view that uses any room takes what the measures leave',()=>{
  const sky={min:300,comfort:500,max:Infinity,touched:0};
  const {sizes}=distribute(2000,[{...text,active:true},sky]);
  expect(sizes[0]).toBe(text.comfort);expect(sizes[1]).toBe(2000-GUTTER-text.comfort);
});

test('when the narrowest measures do not fit, the window touched longest ago folds into a strip',()=>{
  const {sizes,collapsed}=distribute(800,[{...text,active:true},{...card},{...word}]);
  expect(collapsed).toEqual([1]);expect(sizes[1]).toBe(RAIL);
  expect(sizes[0]).toBeGreaterThanOrEqual(text.min);
});

test('a column lifts the area of the work and keeps the others in proportion',()=>{
  const {sizes}=distributeColumn(900,[{active:false},{active:true}],[0.7,0.3]);
  expect(sizes[1]).toBeGreaterThan(sizes[0]);expect(sum(sizes)+GUTTER).toBeLessThanOrEqual(900);
});

test('a split set by hand keeps its fractions; a branch measures as its children add up',()=>{
  expect(pinnedSizes(1007,[{},{}],[0.25,0.75])).toEqual([250,750]);
  expect(combine('row',[text,card]).min).toBe(text.min+card.min+GUTTER);
  expect(combine('column',[text,card]).comfort).toBe(Math.max(text.comfort,card.comfort));
});
