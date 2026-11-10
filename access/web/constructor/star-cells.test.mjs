import {expect,test} from 'vitest';
import {meetsStar,starCells} from './star-cells.mjs';

const overlaps=(a,b,gap)=>a.left<b.right+gap&&a.right>b.left-gap&&a.top<b.bottom+gap&&a.bottom>b.top-gap;
const box=(x,y,width,height)=>({left:x,top:y,right:x+width,bottom:y+height});

test('a place meets a star by the cells exactly when it meets one among all the stars',()=>{
  let seed=11;const random=()=>(seed=(seed*16807)%2147483647)/2147483647;
  for(let round=0;round<40;round++){
    const stars=Array.from({length:300},(_,i)=>({id:'s'+i,...box(random()*1500-60,random()*950-60,18,18)}));
    // A star far out or without a place at all is tested as every star is.
    stars.push({id:'far',...box(1e7,-1e7,18,18)},{id:'lost',...box(NaN,NaN,18,18)},{id:'long',...box(-50,400,6000,18)});
    starCells(stars);
    for(let i=0;i<400;i++){
      const rect=box(random()*1600-100,random()*1000-100,random()*260,random()*60),gap=[0,2,5][i%3],own=i%4?null:stars[Math.floor(random()*stars.length)].id;
      expect(meetsStar(stars,rect,gap,own)).toBe(stars.some(o=>o.id!==own&&overlaps(rect,o,gap)));
    }
    expect(meetsStar(stars,box(-Infinity,0,Infinity,10),2)).toBe(stars.some(o=>overlaps(box(-Infinity,0,Infinity,10),o,2)));
  }
});
