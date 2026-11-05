// The lenses that gather stand in the space as the tree does, each in the form of what it gathers:
//
//   sources   Zarathustra's book as a stair: each part one turn, each chapter a step in the order of reading; the stars a
//             chapter grounds stand out from its step, the text nearest, then the readings, the concepts, the questions.
//             Antiquity as a colonnade: every work a column on an arc ordered by its author's life, its passages rising up
//             the column, the readings of them standing before it, the author at the foot of the works;
//   meaning   a chalice of tiers: the text at its foot, the readings round it, the concepts and images above, the questions
//             at the brim; each circle of the map keeps one sector of every tier, and the links draw what they join toward
//             one another, so a concept stands over the readings and passages it rests on;
//   tensions  scales: a disputed star is the fulcrum; what stands with it (what grounds, develops or echoes it) hangs on
//             one arm and what stands against it on the other, the heavier arm lower; the scales climb round an axis, and a
//             star that hangs on two of them is joined to the second by a thread;
//   images    a dome: the images stand round it in the order of the book, the higher the more concepts they reach; the
//             concepts stand over their images toward the top, and threads lead from each image down to its chapter.
//
// The space's own coordinates: x to the right, y down, z toward the reader. Higher is a smaller y. Each form returns
// {positions: Map(id → [x,y,z]), lines, names, rest, shade?}; the same stars and links lay out the same way every time.

const GOLD='#c9b07a',NEUTRAL='#8ea2ba',RULE='#9fb0c4',AGAINST='#d98f7f';
const TAU=Math.PI*2,GOLDEN=Math.PI*(3-Math.sqrt(5));
const circle=(r,y=0,n=72)=>Array.from({length:n+1},(_,i)=>{const a=i/n*TAU;return [r*Math.cos(a),y,r*Math.sin(a)];});
const byId=(a,b)=>String(a).localeCompare(String(b));

/** The pose each form is first seen from, a little from above. */
export const GATHER_REST=Object.freeze({sources:{yaw:-.22,pitch:-.34},colonnade:{yaw:0,pitch:-.3},meaning:{yaw:-.3,pitch:-.36},tensions:{yaw:-.4,pitch:-.2},images:{yaw:-.15,pitch:-.45}});

/** n places about `step` apart on a small upright disc, the first at its foot: [across, up]. */
function cluster(n,step=12){return Array.from({length:n},(_,j)=>{if(n===1)return [0,0];const r=step*.62*Math.sqrt(j+.5),a=j*GOLDEN+Math.PI/2;return [r*Math.cos(a),r*Math.sin(a)+step*.62*Math.sqrt(n)];});}

/**
 * Zarathustra's sources: the book as a stair. `chapterOf(id)` is {part, chapter} or null; `rungOf(id)` how far a star
 * stands from the text (0 the text … 3 a question).
 */
export function bookSpiral({stars=[],chapterOf=()=>null,rungOf=()=>1,chapterName=chapter=>String(chapter),partName=part=>String(part),radius=260,turn=170}={}){
  const chapters=new Map(),outside=[];
  for(const id of stars){const at=chapterOf(id);if(at&&Number.isFinite(at.chapter)){if(!chapters.has(at.chapter))chapters.set(at.chapter,{part:Number.isFinite(at.part)?at.part:0,ids:[]});chapters.get(at.chapter).ids.push(id);}else outside.push(id);}
  if(!chapters.size)return null;
  // Each part is one turn of the stair, and a chapter stands at its share of its part's chapters.
  const parts=new Map();for(const [chapter,{part}] of chapters){const p=parts.get(part)??{from:chapter,to:chapter};p.from=Math.min(p.from,chapter);p.to=Math.max(p.to,chapter);parts.set(part,p);}
  const order=[...parts.keys()].sort((a,b)=>a-b);
  const turnOf=(part,chapter)=>{const p=parts.get(part);return order.indexOf(part)+(chapter-p.from)/Math.max(1,p.to-p.from+1);};
  const at=(t,r=radius)=>{const a=t*TAU-Math.PI/2;return [r*Math.cos(a),-40-t*turn,r*Math.sin(a)];};
  const positions=new Map(),lines=[],names=[];
  for(const [chapter,{part,ids}] of [...chapters].sort((a,b)=>a[0]-b[0])){
    const t=turnOf(part,chapter),a=t*TAU-Math.PI/2,out=[Math.cos(a),0,Math.sin(a)],across=[-Math.sin(a),0,Math.cos(a)],[sx,sy,sz]=at(t);
    const rungs=new Map();for(const id of ids){const r=Math.max(0,Math.min(3,Math.round(rungOf(id)??1)));if(!rungs.has(r))rungs.set(r,[]);rungs.get(r).push(id);}
    let far=0;
    for(const [r,list] of rungs){
      const d=28+r*36;far=Math.max(far,d);
      list.sort(byId);cluster(list.length).forEach(([c,u],j)=>positions.set(list[j],[sx+out[0]*d+across[0]*c,sy-8-u,sz+out[2]*d+across[2]*c]));
    }
    lines.push({color:GOLD,alpha:.24,weight:1,points:[[sx,sy,sz],[sx+out[0]*far,sy,sz+out[2]*far]]});
    names.push({id:`book:ch:${chapter}`,variant:'tick',align:'end',title:chapterName(chapter),subtitle:'',center:[sx-out[0]*10,sy,sz-out[2]*10]});
  }
  // The stair itself, all its turns, the axis it turns round, and each part named where its turn begins.
  lines.push({color:GOLD,alpha:.55,weight:2,points:Array.from({length:order.length*96+1},(_,i)=>at(i/96))});
  lines.push({color:RULE,alpha:.3,weight:1.2,points:[[0,10,0],[0,at(order.length)[1]-30,0]]});
  lines.push({color:NEUTRAL,alpha:.2,weight:1,points:circle(radius,10)});
  order.forEach((part,i)=>{const [x,y,z]=at(i,radius*.62);names.push({id:`book:part:${part}`,variant:'tick',align:'start',title:partName(part),subtitle:'',center:[x,y-26,z]});});
  // What the book does not hold stands at the foot of the axis.
  outside.sort(byId);cluster(outside.length,16).forEach(([c,u],k)=>positions.set(outside[k],[c,-8-u,0]));
  return {kind:'sources',positions,lines,names,rest:GATHER_REST.sources,shade:{center:[0,10,0],radius:radius+70}};
}

/**
 * Antiquity's sources: a colonnade. `authors` are [{id, works:[{id, up:[ids], front:[ids]}]}] in the order of their lives;
 * an author whose id is null holds the works of no known author. `loose` stars stand in the middle of the arc.
 */
export function colonnade({authors=[],loose=[],radius=640,span=1.3,rise=17}={}){
  const columns=authors.reduce((n,a)=>n+a.works.length,0);
  if(!columns)return null;
  const gap=.7,slots=columns+gap*Math.max(0,authors.length-1);
  const phiOf=slot=>-span+2*span*(slot+.5)/slots;
  const at=(phi,r=radius,y=0)=>[r*Math.sin(phi),y,radius*.55-r*Math.cos(phi)];
  const positions=new Map(),lines=[],names=[];
  let slot=0;
  for(const author of authors){
    const first=slot;
    for(const work of author.works){
      const phi=phiOf(slot++),[x,,z]=at(phi),inward=[-Math.sin(phi),0,Math.cos(phi)];
      positions.set(work.id,[x,-16,z]);
      [...work.up].sort(byId).forEach((id,j)=>positions.set(id,[x,-36-j*rise,z]));
      [...work.front].sort(byId).forEach((id,j)=>positions.set(id,[x+inward[0]*30,-30-j*rise,z+inward[2]*30]));
      const height=36+Math.max(work.up.length,work.front.length,1)*rise;
      lines.push({color:GOLD,alpha:.42,weight:1.6,points:[[x,0,z],[x,-height,z]]});
    }
    // An author stands before the plinth of their works.
    const last=slot-1,mid=(phiOf(first)+phiOf(last))/2;
    if(author.id)positions.set(author.id,at(mid,radius*.86,20));
    lines.push({color:RULE,alpha:.5,weight:2.4,points:Array.from({length:9},(_,i)=>at(phiOf(first-.4)+(phiOf(last+.4)-phiOf(first-.4))*i/8))});
    slot+=gap;
  }
  lines.push({color:RULE,alpha:.22,weight:1,points:Array.from({length:61},(_,i)=>at(-span-.06+(2*span+.12)*i/60,radius*1.03))});
  [...loose].sort(byId).forEach((id,i)=>positions.set(id,[(i%6-2.5)*26,-10-Math.floor(i/6)*20,radius*.3]));
  return {kind:'sources',positions,lines,names,rest:GATHER_REST.colonnade};
}

/**
 * Meaning as a chalice of tiers: the text at its foot, the readings round it, the concepts and images above, the questions
 * at the brim. `tierOf(id)` is a star's tier (0 the text … 3 the questions) or null to leave it out; `groupOf(id)` the circle
 * of the map it belongs to, which keeps one sector of every tier; `links` are {from, to}, and a link draws the stars it
 * joins toward one another round the chalice. `groupNames` name the sectors at the brim.
 */
export function meaningTiers({stars=[],tierOf=()=>null,groupOf=()=>null,links=[],tierNames=[],groupNames=new Map(),iterations=160}={}){
  const ids=stars.filter(id=>Number.isFinite(tierOf(id))).sort(byId);
  if(!ids.length)return null;
  const index=new Map(ids.map((id,i)=>[id,i])),tier=ids.map(id=>tierOf(id)),keys=[...new Set(tier)].sort((a,b)=>a-b);
  const edges=[];for(const l of links){const a=index.get(l.from),b=index.get(l.to);if(a!==undefined&&b!==undefined&&a!==b)edges.push([a,b]);}
  const near=ids.map(()=>[]);for(const [a,b] of edges){near[a].push(b);near[b].push(a);}
  // Each circle of the map keeps the same sector of every tier, as wide as its share of the whole.
  const counts=new Map();ids.forEach(id=>{const g=String(groupOf(id)??'');counts.set(g,(counts.get(g)??0)+1);});
  const sectors=new Map();let from=0;
  for(const [g,n] of [...counts].sort((a,b)=>b[1]-a[1]||byId(a[0],b[0]))){const width=n/ids.length*TAU;sectors.set(g,[from,from+width]);from+=width;}
  const groupAt=ids.map(id=>String(groupOf(id)??''));
  // A tier is a ring, wider and higher the farther it stands from the text; a crowded ring takes several rows like seats.
  const shape=new Map();
  let wider=0;
  for(const t of keys){const n=tier.filter(v=>v===t).length,radius=Math.max(70+t*120,wider+90,n*15/(TAU*3)),rows=Math.max(1,Math.min(3,Math.ceil(n*15/(TAU*radius))));shape.set(t,{n,radius,rows,y:-t*120});wider=radius;}
  // A star starts in its circle's sector, in the order a walk along its links meets it, and keeps near that start.
  const angle=new Array(ids.length),home=new Array(ids.length);
  for(const t of keys){
    const own=ids.map((_,i)=>i).filter(i=>tier[i]===t),bySector=new Map();
    for(const i of own){if(!bySector.has(groupAt[i]))bySector.set(groupAt[i],[]);bySector.get(groupAt[i]).push(i);}
    for(const [g,members] of bySector){
      const seen=new Set(),walk=[];
      for(const start of members){if(seen.has(start))continue;const queue=[start];seen.add(start);while(queue.length){const v=queue.shift();walk.push(v);for(const w of [...near[v]].sort((a,b)=>a-b))if(!seen.has(w)&&tier[w]===t&&groupAt[w]===g){seen.add(w);queue.push(w);}}}
      const [a0,a1]=sectors.get(g);walk.forEach((i,j)=>{angle[i]=a0+(a1-a0)*(j+.5)/walk.length;home[i]=angle[i];});
    }
  }
  const wrap=a=>Math.atan2(Math.sin(a),Math.cos(a));
  for(let it=0;it<iterations;it++){
    const cool=1-it/iterations,move=new Array(ids.length).fill(0);
    // A star with many links is drawn by their mean, so that no hub pulls the whole ring to itself.
    for(const [a,b] of edges){const d=wrap(angle[b]-angle[a])*.08*cool;move[a]+=d/near[a].length;move[b]-=d/near[b].length;}
    for(let i=0;i<ids.length;i++)move[i]+=wrap(home[i]-angle[i])*.05*cool;
    for(let i=0;i<ids.length;i++)angle[i]+=Math.max(-.08,Math.min(.08,move[i]));
    // Neighbours on a ring keep their room.
    for(const t of keys){
      const {radius,rows}=shape.get(t),gap=13/radius/rows,own=ids.map((_,i)=>i).filter(i=>tier[i]===t).sort((a,b)=>wrap(angle[a])-wrap(angle[b])||a-b);
      for(let k=0;k<own.length;k++){const a=own[k],b=own[(k+1)%own.length];if(a===b)continue;const d=wrap(angle[b]-angle[a]);if(d<gap&&d>-Math.PI/2){const push=(gap-d)/2;angle[a]-=push;angle[b]+=push;}}
    }
  }
  const positions=new Map(),lines=[],names=[];
  for(const t of keys){
    const {radius,rows,y}=shape.get(t),own=ids.map((_,i)=>i).filter(i=>tier[i]===t).sort((a,b)=>wrap(angle[a])-wrap(angle[b])||a-b);
    own.forEach((i,k)=>{const row=rows>1?k%rows-(rows-1)/2:0,r=radius+row*17,a=angle[i];positions.set(ids[i],[r*Math.cos(a),y-row*9,r*Math.sin(a)]);});
    lines.push({color:NEUTRAL,alpha:.16+.04*t,weight:1.1,points:circle(radius,y+6,96)});
    if(tierNames[t])names.push({id:`meaning:tier:${t}`,variant:'tick',align:'start',title:tierNames[t],subtitle:'',center:[radius+18*rows+10,y+6,0]});
  }
  // The chalice's walls: a line from the foot to the brim on each border between circles, and each circle named at the brim.
  const top=keys[keys.length-1],brim=shape.get(top),foot=shape.get(keys[0]);
  for(const [g,[a0,a1]] of sectors){
    if(sectors.size>1)lines.push({color:NEUTRAL,alpha:.12,weight:1,points:keys.map(t=>{const {radius,y}=shape.get(t);return [radius*Math.cos(a0),y+6,radius*Math.sin(a0)];})});
    const title=groupNames.get?.(g);if(!title)continue;const mid=(a0+a1)/2,r=brim.radius+18*brim.rows+26;
    names.push({id:`meaning:circle:${g}`,variant:'tick',title,subtitle:'',center:[r*Math.cos(mid),brim.y-20,r*Math.sin(mid)]});
  }
  lines.push({color:RULE,alpha:.3,weight:1.2,points:[[0,foot.y+40,0],[0,brim.y-30,0]]});
  return {kind:'meaning',positions,lines,names,rest:GATHER_REST.meaning,shade:{center:[0,40,0],radius:foot.radius+90}};
}

/**
 * Tensions as scales. `pivots` are [{id, grounds:[ids], against:[ids]}]: what stands with a disputed star hangs on one
 * arm, what stands against it on the other. At most `most` scales, the most disputed first.
 */
export function tensionScales({pivots=[],most=28,radius=230,row=34,arm=22,hang=5}={}){
  // The most disputed star first; a star that already hangs on a scale stays there and is not a fulcrum of its own.
  const candidates=pivots.map(p=>({id:p.id,grounds:[...new Set(p.grounds??[])].filter(id=>id!==p.id).sort(byId),against:[...new Set(p.against??[])].filter(id=>id!==p.id).sort(byId)})).filter(p=>p.against.length).sort((a,b)=>b.against.length-a.against.length||b.grounds.length-a.grounds.length||byId(a.id,b.id));
  const list=[],hung=new Set();
  for(const p of candidates){if(list.length>=most)break;if(hung.has(p.id))continue;list.push(p);hung.add(p.id);for(const id of [...p.grounds.slice(0,hang),...p.against.slice(0,hang)])hung.add(id);}
  if(!list.length)return null;
  const positions=new Map(),lines=[],height=(list.length-1)*row;
  const centre=i=>{const a=i*TAU/7.5-Math.PI/2;return {a,c:[radius*Math.cos(a),-height/2+i*row,radius*Math.sin(a)]};};
  // The fulcrums first, so that a disputed star always stands at its own scale.
  list.forEach((p,i)=>positions.set(p.id,centre(i).c));
  list.forEach((p,i)=>{
    const {a,c}=centre(i),u=[-Math.sin(a),0,Math.cos(a)];
    const {grounds,against}=p;
    const ng=Math.min(hang,grounds.length),na=Math.min(hang,against.length),L=arm*Math.max(ng,na,1);
    // The heavier arm hangs lower: what stands against on the right, what stands with on the left.
    const tilt=Math.max(-12,Math.min(12,(against.length-grounds.length)*3));
    const beam=d=>[c[0]+u[0]*d,c[1]+tilt*d/L,c[2]+u[2]*d];
    lines.push({color:GOLD,alpha:.6,weight:1.8,points:[beam(-L),c,beam(L)]});
    lines.push({color:RULE,alpha:.3,weight:1,points:[c,[c[0],c[1]+10,c[2]]]});
    const hangOn=(id,d,color)=>{
      const from=beam(d);
      if(positions.has(id)){lines.push({color,alpha:.1,weight:1,points:[from,positions.get(id)]});return;}
      const to=[from[0],from[1]+16,from[2]];positions.set(id,to);lines.push({color,alpha:.34,weight:1,points:[from,to]});
    };
    grounds.slice(0,ng).forEach((id,j)=>hangOn(id,-arm*(j+1),NEUTRAL));
    against.slice(0,na).forEach((id,j)=>hangOn(id,arm*(j+1),AGAINST));
  });
  lines.push({color:RULE,alpha:.3,weight:1.2,points:[[0,-height/2-50,0],[0,height/2+50,0]]});
  lines.push({color:NEUTRAL,alpha:.16,weight:1,points:Array.from({length:Math.ceil(list.length/7.5*96)+1},(_,i)=>{const t=i/96,a=t*TAU-Math.PI/2;return [radius*Math.cos(a),-height/2+t*7.5*row,radius*Math.sin(a)];})});
  return {kind:'tensions',positions,lines,names:[],rest:GATHER_REST.tensions,lead:new Set(list.map(p=>p.id))};
}

/**
 * Images on a dome. `images` are [{id, chapter, reach}] (reach 0…1: how far the image rises toward concepts); `concepts` are
 * [{id, images:[image ids], low}] (low for a figure, which stands under the concepts). `chapterName(chapter)` names a
 * chapter on the rim, `marks` are [{id, chapter, title}] for the parts.
 */
export function imageDome({images=[],concepts=[],maxChapter=80,chapterName=String,marks=[],radius=380}={}){
  if(!images.length)return null;
  const positions=new Map(),lines=[],names=[],place=new Map();
  const at=(lon,lat,r=radius)=>[r*Math.cos(lat)*Math.cos(lon),-r*Math.sin(lat),r*Math.cos(lat)*Math.sin(lon)];
  const lonOfChapter=chapter=>chapter/Math.max(1,maxChapter)*TAU-Math.PI/2;
  // The images stand round the dome in the order of the book, evenly; a thread leads down to the chapter on the rim.
  const ordered=[...images].sort((a,b)=>(a.chapter??Infinity)-(b.chapter??Infinity)||byId(a.id,b.id)),rims=new Set();
  ordered.forEach((image,i)=>{
    const lon=i/ordered.length*TAU-Math.PI/2,lat=.22+.75*Math.max(0,Math.min(1,image.reach??0));
    place.set(image.id,{lon,lat});positions.set(image.id,at(lon,lat));
    if(!Number.isFinite(image.chapter))return;
    let end=lonOfChapter(image.chapter);while(end-lon>Math.PI)end-=TAU;while(lon-end>Math.PI)end+=TAU;
    lines.push({color:GOLD,alpha:.3,weight:1,points:Array.from({length:25},(_,k)=>at(lon+(end-lon)*k/24,lat*(1-k/24)))});
    if(!rims.has(image.chapter)){rims.add(image.chapter);const [x,,z]=at(lonOfChapter(image.chapter),0,radius+20);names.push({id:`dome:ch:${image.chapter}`,variant:'tick',title:chapterName(image.chapter),subtitle:'',center:[x,6,z]});}
  });
  // A concept stands over the images that reach it, the higher the more of the dome they span.
  const taken=[];
  for(const concept of [...concepts].sort((a,b)=>b.images.length-a.images.length||byId(a.id,b.id))){
    const own=concept.images.map(id=>place.get(id)).filter(Boolean);if(!own.length)continue;
    const sx=own.reduce((s,p)=>s+Math.cos(p.lon),0)/own.length,sz=own.reduce((s,p)=>s+Math.sin(p.lon),0)/own.length,spread=1-Math.hypot(sx,sz);
    let lon=Math.atan2(sz,sx),lat=Math.min(1.45,Math.max(...own.map(p=>p.lat))+(concept.low?.1:.26)+spread*.5);
    for(let k=0;k<40&&taken.some(p=>Math.abs(p.lat-lat)<.07&&Math.abs(Math.atan2(Math.sin(p.lon-lon),Math.cos(p.lon-lon)))*Math.cos(lat)<.08);k++)lon+=.09/Math.max(.25,Math.cos(lat));
    taken.push({lon,lat});positions.set(concept.id,at(lon,lat,radius*.94));
  }
  lines.push({color:RULE,alpha:.45,weight:1.4,points:circle(radius,0,120)});
  for(const lat of [.5,1.0])lines.push({color:NEUTRAL,alpha:.12,weight:1,points:Array.from({length:121},(_,i)=>at(i/120*TAU,lat))});
  for(let m=0;m<6;m++)lines.push({color:NEUTRAL,alpha:.09,weight:1,points:Array.from({length:41},(_,i)=>at(m/6*TAU,i/40*Math.PI/2))});
  for(const mark of marks){const [x,,z]=at(lonOfChapter(mark.chapter),0,radius+44);names.push({id:`dome:${mark.id}`,variant:'tick',title:mark.title,subtitle:'',center:[x,24,z]});lines.push({color:RULE,alpha:.4,weight:1.2,points:[at(lonOfChapter(mark.chapter),0,radius-10),at(lonOfChapter(mark.chapter),0,radius+30)]});}
  return {kind:'images',positions,lines,names,rest:GATHER_REST.images,shade:{center:[0,0,0],radius:radius*.8},lead:new Set(positions.keys())};
}
