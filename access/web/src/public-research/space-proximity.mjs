// The lens "Proximity" measures nearness from one chosen star, by one named
// criterion at a time, and says which. The distance from the centre is the
// measure; the direction only groups (a branch or a circle of the map gets its
// own sector). A star the criterion cannot measure — no shared concept to
// compare, no path, no date, no place — stands on an outer ring that says so,
// rather than at a distance the data does not give.

import {greatCircleKm} from './space-geography.mjs';

const lang=locale=>String(locale||'ru').startsWith('en')?'en':'ru';
const GAP=11,ROW=7;
const pick=(value,locale)=>typeof value==='string'?value:value?.[lang(locale)]??value?.en??value?.ru??'';

/** 1 − |A ∩ B| / |A ∪ B| for two sets of keys; null when either is empty. */
export function jaccardDistance(a,b){
  const left=new Set(a??[]),right=new Set(b??[]);if(!left.size||!right.size)return null;
  let shared=0;for(const key of left)if(right.has(key))shared++;
  return 1-shared/(left.size+right.size-shared);
}

/** Fewest undirected steps from `start` to every star it reaches. */
export function hopsFrom(start,edges){
  const next=new Map();
  for(const [a,b] of edges){if(!next.has(a))next.set(a,new Set());if(!next.has(b))next.set(b,new Set());next.get(a).add(b);next.get(b).add(a);}
  const seen=new Map([[start,0]]),queue=[start];
  for(let i=0;i<queue.length;i++){const id=queue[i];for(const other of next.get(id)??[])if(!seen.has(other)){seen.set(other,seen.get(id)+1);queue.push(other);}}
  return seen;
}

/** The middle of a span of years, or null. */
export const spanMiddle=span=>span&&Number.isFinite(span.from)&&Number.isFinite(span.to)?(span.from+span.to)/2:null;

/** Rings at round values of a measure up to `max`, at most `count` of them. */
export function ringValues(max,count=4){
  if(!(max>0))return [];
  const raw=max/count,power=10**Math.floor(Math.log10(raw)),step=[1,2,2.5,5,10].map(f=>f*power).find(value=>value>=raw)??10*power;
  const out=[];for(let value=step;value<=max*1.0001;value+=step)out.push(Number(value.toPrecision(6)));
  return out;
}

/**
 * Lay out `stars` around `center` by `distance(center, id)` (a number or null).
 * `groupOf(id)` gives the sector a star belongs to, `groups` their order and names.
 */
export function proximityLayout({center,stars=[],distance=()=>null,groupOf=()=>null,groups=[],radius=420,integer=false}={}){
  const others=stars.filter(id=>id!==center),measured=new Map(),unmeasured=[];
  for(const id of others){const value=distance(id);if(Number.isFinite(value))measured.set(id,value);else unmeasured.push(id);}
  // Nearest is a small inner ring, not the centre itself, so a star at distance zero never hides the chosen one.
  const max=Math.max(0,...measured.values()),outer=radius+64,inner=38;
  const r=value=>inner+(max>0?value/max:0)*(radius-inner);
  // Sectors: each group takes a share of the circle by its size, never less than a sliver.
  const sizes=new Map(groups.map(group=>[group.id,0]));for(const id of others){const key=groupOf(id)??'__other';sizes.set(key,(sizes.get(key)??0)+1);}
  const order=[...sizes.keys()].filter(key=>sizes.get(key)>0),total=order.reduce((sum,key)=>sum+Math.max(sizes.get(key),2),0);
  const sectors=new Map();let start=-Math.PI/2;
  for(const key of order){const span=2*Math.PI*Math.max(sizes.get(key),2)/total;sectors.set(key,[start,start+span]);start+=span;}
  const positions=new Map([[center,[0,0,0]]]);
  for(const key of order){
    const [a,b]=sectors.get(key),members=others.filter(id=>(groupOf(id)??'__other')===key).sort((x,y)=>(measured.get(x)??Infinity)-(measured.get(y)??Infinity));
    // Stars at one distance share its ring across the sector; where the arc is too short for them all,
    // the rest stand in short rows just outside it, so a crowded ring stays legible and its measure near.
    const rings=new Map();
    for(const id of members){const d=measured.has(id)?r(measured.get(id)):outer,key=Math.round(d/4);if(!rings.has(key))rings.set(key,{d,ids:[]});rings.get(key).ids.push(id);}
    for(const {d,ids} of rings.values()){
      const capacity=Math.max(1,Math.floor(d*(b-a)/GAP));
      ids.forEach((id,index)=>{
        const row=Math.floor(index/capacity),inRow=Math.min(capacity,ids.length-row*capacity),slot=index%capacity;
        const angle=a+(slot+.5)/inRow*(b-a),rr=d+row*ROW;
        positions.set(id,[rr*Math.cos(angle),rr*Math.sin(angle),0]);
      });
    }
  }
  // Whole measures (steps, chapters) take whole rings.
  const step=integer?Math.max(1,Math.ceil(max/4)):0,rings=integer?Array.from({length:Math.floor(max/step)},(_,index)=>(index+1)*step):ringValues(max);
  return {
    kind:'proximity',center,positions,max,radius,outer,inner,bounds:{x:[-outer-40,outer+40],y:[-outer-40,outer+40]},rows:{axis:22},strip:26,
    distanceOf:id=>measured.get(id)??null,unmeasured,sectors,
    labels(locale,{unit={ru:'',en:''},format=value=>String(value),unmeasuredLabel={ru:'не измеряется',en:'not measured'}}={}){
      const out=[{id:'near:ring:0',variant:'ring',radius:inner,title:'',subtitle:'',center:[0,0,0]},{id:'near:tick:0',variant:'tick',title:`${format(0)}${pick(unit,locale)?` ${pick(unit,locale)}`:''}`,subtitle:'',center:[0,-inner-9,0]}];
      for(const value of rings){
        out.push({id:`near:ring:${value}`,variant:'ring',radius:r(value),title:'',subtitle:'',center:[0,0,0]});
        out.push({id:`near:tick:${value}`,variant:'tick',title:`${format(value)}${pick(unit,locale)?` ${pick(unit,locale)}`:''}`,subtitle:'',center:[0,-r(value)-9,0]});
      }
      if(unmeasured.length){
        out.push({id:'near:ring:outer',variant:'ring',tone:'strong',radius:outer,title:'',subtitle:'',center:[0,0,0]});
        out.push({id:'near:tick:outer',variant:'tick',title:`${pick(unmeasuredLabel,locale)} · ${unmeasured.length}`,subtitle:'',center:[0,-outer-9,0]});
      }
      // Each sector is named at its middle, outside the rings: the direction groups and measures nothing.
      for(const [key,[a,b]] of sectors){
        const group=groups.find(item=>item.id===key);if(!group)continue;
        const angle=(a+b)/2,d=outer+26,x=d*Math.cos(angle),y=d*Math.sin(angle);
        out.push({id:`near:sector:${key}`,variant:'tick',align:x>12?'start':x<-12?'end':'',title:pick(group.title,locale),subtitle:'',center:[x,y,0],maxWidth:180});
      }
      return out;
    },
  };
}

/** The distance functions a profile offers, each named; `null` where the data cannot measure a pair. */
export function proximityCriteria({conceptsOf=()=>null,edges=[],spanOf=()=>null,pointOf=()=>null,chapterOf=()=>null,groundsOf=()=>null}={}){
  return {
    concepts:center=>id=>jaccardDistance(conceptsOf(center),conceptsOf(id)),
    grounds:center=>id=>jaccardDistance(groundsOf(center),groundsOf(id)),
    relations:center=>{const hops=hopsFrom(center,edges);return id=>hops.has(id)?hops.get(id):null;},
    time:center=>{const here=spanMiddle(spanOf(center));return id=>{const there=spanMiddle(spanOf(id));return here===null||there===null?null:Math.abs(there-here);};},
    chapters:center=>{const here=chapterOf(center);return id=>{const there=chapterOf(id);return Number.isFinite(here)&&Number.isFinite(there)?Math.abs(there-here):null;};},
    place:center=>{const here=pointOf(center);return id=>{const there=pointOf(id);return here&&there?greatCircleKm(here,there):null;};},
  };
}
