// Scale labels occupy actual screen rectangles; leave a little air between them.
export function scaleLabelClear(area,labels,obstacles=[]){
  const overlaps=(a,b,gap)=>a.left<b.right+gap&&a.right>b.left-gap&&a.top<b.bottom+gap&&a.bottom>b.top-gap;
  return !labels.some(other=>overlaps(area,other,2))&&!obstacles.some(other=>overlaps(area,other,0));
}
