// The breathing of the bench: how the room of a row or a column is shared between its windows.
// Each view knows its measure: the narrowest it can be read at, the width it reads best at, and the widest a line of
// it should run (a text keeps its line and takes the rest as margin). The window in which the work goes on gets its
// good measure first; what is left goes to the views that can use any room. A split the reader set by hand keeps
// the shares the hand gave it. When even the narrowest measures do not fit, the window touched longest ago folds
// into a strip.

export const RAIL=38,GUTTER=7;

// Measures in characters of the window's own text; `pad` is the window's own frame around the text.
const MEASURES=Object.freeze({
  text:{min:36,comfort:66,max:76,pad:72},
  parallel:{min:62,comfort:118,max:140,pad:96},
  card:{min:34,comfort:54,max:64,pad:56},
  fragment:{min:34,comfort:56,max:68,pad:56},
  word:{min:26,comfort:36,max:44,pad:44},
  thought:{min:34,comfort:58,max:70,pad:56},
});

/** A view's measure in pixels, from the width of one character of its text. */
export function measureOf(view,{ch=8.6,parallel=false}={}){
  const m=MEASURES[view==='text'&&parallel?'parallel':view]??MEASURES.card;
  return {min:Math.round(m.min*ch+m.pad),comfort:Math.round(m.comfort*ch+m.pad),max:Math.round(m.max*ch+m.pad)};
}

/** The measure of a branch of the layout: a row adds its children up, a column takes the widest of them. */
export function combine(dir,items){
  const open=items.filter(item=>!item.collapsed);
  if(dir==='row'){
    const gutters=GUTTER*Math.max(0,items.length-1),rails=items.filter(item=>item.collapsed).length*RAIL;
    const sum=key=>open.reduce((total,item)=>total+(Number.isFinite(item[key])?item[key]:0),0);
    return {min:sum('min')+gutters+rails,comfort:sum('comfort')+gutters+rails,max:open.some(item=>!Number.isFinite(item.max))?Infinity:sum('max')+gutters+rails};
  }
  const most=key=>open.reduce((m,item)=>Math.max(m,item[key]),0);
  return {min:most('min'),comfort:most('comfort'),max:open.some(item=>!Number.isFinite(item.max))?Infinity:most('max')};
}

/**
 * Widths for the children of a row. `items[i]` has min, comfort and max (max may be Infinity for a view that uses
 * any room), `collapsed`, `active` (the work goes on there) and `touched` (when it was last worked in).
 * Returns {sizes, collapsed}: a width for each child (a strip for a folded one) and the children folded to fit.
 */
export function distribute(total,items){
  const n=items.length;if(!n)return {sizes:[],collapsed:[]};
  const folded=items.map(item=>!!item.collapsed);
  const room=()=>total-GUTTER*(n-1)-folded.filter(Boolean).length*RAIL;
  const need=()=>items.reduce((sum,item,i)=>sum+(folded[i]?0:item.min),0);
  // Fold the window touched longest ago until the narrowest measures fit; the active window stays open.
  while(need()>room()){
    const candidates=items.map((item,i)=>({i,item})).filter(({i,item})=>!folded[i]&&!item.active&&item.fold!==false);
    if(!candidates.length)break;
    candidates.sort((a,b)=>(a.item.touched??0)-(b.item.touched??0));folded[candidates[0].i]=true;
  }
  const open=items.map((item,i)=>i).filter(i=>!folded[i]);
  const sizes=items.map((item,i)=>folded[i]?RAIL:0);
  let left=Math.max(0,room());
  if(!open.length)return {sizes,collapsed:folded.map((v,i)=>v&&!items[i].collapsed?i:-1).filter(i=>i>=0)};
  // Too narrow even for the minimums: every open window shares the room by its minimum.
  if(need()>left){const scale=left/need();for(const i of open)sizes[i]=Math.floor(items[i].min*scale);return finish();}
  for(const i of open){sizes[i]=items[i].min;left-=items[i].min;}
  const give=(indexes,cap)=>{
    // Room is given out in proportion to what each still lacks, so no window overshoots its target.
    for(let round=0;round<8&&left>0.5;round++){
      const lacking=indexes.filter(i=>cap(i)-sizes[i]>0.5);if(!lacking.length)break;
      const want=lacking.reduce((sum,i)=>sum+(cap(i)-sizes[i]),0),share=Math.min(1,left/want);
      for(const i of lacking){const add=(cap(i)-sizes[i])*share;sizes[i]+=add;left-=add;}
    }
  };
  const active=open.filter(i=>items[i].active),rest=open.filter(i=>!items[i].active);
  give(active,i=>items[i].comfort);
  give(rest,i=>items[i].comfort);
  const elastic=open.filter(i=>!Number.isFinite(items[i].max));
  if(elastic.length&&left>0.5){const each=left/elastic.length;for(const i of elastic)sizes[i]+=each;left=0;}
  give([...active,...rest].filter(i=>Number.isFinite(items[i].max)),i=>items[i].max);
  // What no measure asks for is shared by all, the active window first; a text takes it as its margins.
  if(left>0.5){const takers=open;const each=left/takers.length;for(const i of takers)sizes[i]+=each;left=0;}
  return finish();
  function finish(){
    const rounded=sizes.map(value=>Math.max(0,Math.floor(value)));
    return {sizes:rounded,collapsed:folded.map((value,i)=>value&&!items[i].collapsed?i:-1).filter(i=>i>=0)};
  }
}

/**
 * Heights for the children of a column. Height has no reading measure: the shares stand as set, and the area in
 * which the work goes on grows to at least `lift` of the room, the others keeping their proportions.
 */
export function distributeColumn(total,items,shares,{lift=0.58,min=120}={}){
  const n=items.length;if(!n)return {sizes:[],collapsed:[]};
  const room=total-GUTTER*(n-1)-items.filter(item=>item.collapsed).length*RAIL;
  const open=items.map((item,i)=>i).filter(i=>!items[i].collapsed);
  const base=open.map(i=>shares[i]??1/n),sum=base.reduce((a,b)=>a+b,0)||1;
  let parts=base.map(value=>value/sum);
  const at=open.findIndex(i=>items[i].active);
  if(at>=0&&open.length>1&&parts[at]<lift){
    const others=1-parts[at],scale=others>0?(1-lift)/others:0;parts=parts.map((value,k)=>k===at?lift:value*scale);
  }
  const sizes=items.map(item=>item.collapsed?RAIL:0);
  open.forEach((i,k)=>{sizes[i]=Math.max(min,Math.floor(room*parts[k]));});
  const over=sizes.reduce((a,b)=>a+b,0)+GUTTER*(n-1)-total;
  if(over>0&&open.length){const biggest=open.reduce((a,b)=>sizes[a]>=sizes[b]?a:b);sizes[biggest]=Math.max(min,sizes[biggest]-over);}
  return {sizes,collapsed:[]};
}

/** Shares by hand: the pixel widths a pinned split keeps, from its stored fractions. */
export function pinnedSizes(total,items,shares){
  const n=items.length,room=total-GUTTER*(n-1)-items.filter(item=>item.collapsed).length*RAIL;
  const open=items.map((item,i)=>i).filter(i=>!items[i].collapsed);
  const sum=open.reduce((s,i)=>s+(shares[i]??1/n),0)||1;
  return items.map((item,i)=>item.collapsed?RAIL:Math.floor(room*(shares[i]??1/n)/sum));
}
