// The stars' boxes by cells of the screen, so a name's place is tested against the stars near it alone. Whether a box
// meets any star but its own (sky.mjs's `overlaps`, with the gap) has the answer a test against every star has.
const CELL=48;
const cellOf=(x,y)=>(x+4096)*8192+(y+4096);
const overlaps=(a,b,gap)=>a.left<b.right+gap&&a.right>b.left-gap&&a.top<b.bottom+gap&&a.bottom>b.top-gap;
// A box too wide for its cells to count, or out of the screen's numbers, is tested against every star.
const spanOf=(low,high)=>{const a=Math.floor(low/CELL),b=Math.floor(high/CELL);return b-a<=64&&Math.abs(a)<4000&&Math.abs(b)<4000?[a,b]:null;};

// Index a list of star boxes ({id, left, top, right, bottom}) in place and return it.
export function starCells(stars){
  const cells=new Map(),wide=[];
  for(const o of stars){
    const xs=spanOf(o.left,o.right),ys=spanOf(o.top,o.bottom);if(!xs||!ys){wide.push(o);continue;}
    for(let x=xs[0];x<=xs[1];x++)for(let y=ys[0];y<=ys[1];y++){const key=cellOf(x,y);let cell=cells.get(key);if(!cell)cells.set(key,cell=[]);cell.push(o);}
  }
  stars.cells=cells;stars.wide=wide;return stars;
}

export function meetsStar(stars,rect,gap,ownId=null){
  const xs=stars.cells&&spanOf(rect.left-gap,rect.right+gap),ys=xs&&spanOf(rect.top-gap,rect.bottom+gap);
  if(!ys)return stars.some(o=>o.id!==ownId&&overlaps(rect,o,gap));
  for(const o of stars.wide)if(o.id!==ownId&&overlaps(rect,o,gap))return true;
  for(let x=xs[0];x<=xs[1];x++)for(let y=ys[0];y<=ys[1];y++){
    const cell=stars.cells.get(cellOf(x,y));if(cell)for(const o of cell)if(o.id!==ownId&&overlaps(rect,o,gap))return true;
  }
  return false;
}

// How many stars a box meets (with the gap), the number a count over every star gives. A star lying in several cells
// is counted once.
export function countStars(stars,rect,gap){
  const xs=stars.cells&&spanOf(rect.left-gap,rect.right+gap),ys=xs&&spanOf(rect.top-gap,rect.bottom+gap);
  if(!ys)return stars.reduce((sum,o)=>sum+(overlaps(rect,o,gap)?1:0),0);
  let count=0;const seen=new Set();
  for(const o of stars.wide)if(overlaps(rect,o,gap))count++;
  for(let x=xs[0];x<=xs[1];x++)for(let y=ys[0];y<=ys[1];y++){
    const cell=stars.cells.get(cellOf(x,y));if(cell)for(const o of cell)if(!seen.has(o)&&overlaps(rect,o,gap)){seen.add(o);count++;}
  }
  return count;
}
