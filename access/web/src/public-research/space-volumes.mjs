// The lenses that measure stand in the space as the tree does. Each keeps its measure readable — its scale is drawn in the
// space itself, as lines on the ground and names at their marks — and gives the third dimension a sense of its own:
//
//   time       a landscape of years: the scale runs across, its rows (branches, circles) recede in depth, stars that share a
//              year rise in a small column, and the eras stand in bands before the scale;
//   space      a cube of place and time: the map lies on the ground, the height is the time (years, or the book's order), so a
//              star stands over its place at the height of its time and a place's stars make a column over it;
//   proximity  a sphere round the chosen star: the distance is the radius, the direction its group, as on the flat rings,
//              and the height is the time: what came earlier lies below, later above.
//
// The space's own coordinates: x to the right, y down, z toward the reader. Higher is a smaller y.

const NEUTRAL='#8ea2ba',RULE='#9fb0c4',GOLD='#c9b07a';
const circle=(r,y,n=72,cx=0,cz=0)=>Array.from({length:n+1},(_,i)=>{const a=i/n*Math.PI*2;return [cx+r*Math.cos(a),y,cz+r*Math.sin(a)];});
const nearestLane=(lanes,y)=>lanes.reduce((best,lane,index)=>Math.abs(lane.y-y)<Math.abs(lanes[best].y-y)?index:best,0);

/** The rest a lens's volume is seen from: a little from above and from its left, so its depth reads at once. */
export const VOLUME_REST=Object.freeze({time:{yaw:-.24,pitch:-.5},space:{yaw:-.5,pitch:-.58},proximity:{yaw:-.3,pitch:-.5}});

/**
 * Time as a landscape. `timeline` is the flat scale (space-timeline.mjs); its labels give the marks.
 * Returns positions, the lines under the stars and the names to draw.
 */
export function timeVolume(timeline,{locale='ru',depth=900,rise=1,tower=170}={}){
  if(!timeline?.positions?.size||!timeline.lanes?.length)return null;
  const lanes=timeline.lanes,n=lanes.length,gap=Math.min(76,depth/Math.max(1,n-1||1)),zOf=index=>(index-(n-1)/2)*gap;
  const front=zOf(n-1)+gap*.6,back=zOf(0)-gap*.5;
  const laneOf=new Map(),columns=new Map();
  for(const [id,[x,y]] of timeline.positions){
    const index=nearestLane(lanes,y);laneOf.set(id,index);
    // Stars of one row at one mark of the scale make a column; they keep the order they had on the flat row.
    const key=`${index}:${Math.round(x/6)}`;if(!columns.has(key))columns.set(key,[]);columns.get(key).push([id,y]);
  }
  const tallest=Math.max(1,...[...columns.values()].map(list=>list.length)),step=Math.min(15,tower/tallest)*rise;
  const positions=new Map();
  // A star keeps its year across; its row recedes in depth; stars that share a mark rise in a column over it.
  for(const list of columns.values())list.sort((a,b)=>a[1]-b[1]).forEach(([id],k)=>{const [x]=timeline.positions.get(id);positions.set(id,[x,-8-k*step,zOf(laneOf.get(id))]);});
  const xs=[...positions.values()].map(p=>p[0]),[x0,x1]=timeline.bounds??[Math.min(...xs),Math.max(...xs)];
  const lines=[],names=[];
  // Each row is a thread on the ground across the years its stars span.
  lanes.forEach((lane,index)=>{
    const own=[...positions].filter(([id])=>laneOf.get(id)===index).map(([,p])=>p[0]);if(!own.length)return;
    const from=Math.min(...own),to=Math.max(...own);
    lines.push({color:NEUTRAL,alpha:.42,weight:1.4,points:[[from-12,0,zOf(index)],[to+12,0,zOf(index)]]});
  });
  // The ground's frame and its front edge, where the scale is read.
  lines.push({color:RULE,alpha:.32,weight:1,points:[[x0-30,0,back],[x1+30,0,back],[x1+30,0,front],[x0-30,0,front],[x0-30,0,back]]});
  for(const label of timeline.labels(locale)){
    const [x]=label.center??[0];
    if(label.variant==='rule')lines.push({color:RULE,alpha:label.tone==='strong'?.55:.22,weight:label.tone==='strong'?1.6:1,points:[[x,0,back],[x,0,front]]});
    else if(label.variant==='axis')names.push({id:label.id,variant:'tick',title:label.title,subtitle:'',center:[x,8,front+14]});
    else if(label.variant==='side')names.push({id:label.id,variant:'tick',align:label.align,title:label.title,subtitle:'',center:[x,8,front+30]});
    else if(label.variant==='era'&&label.span){
      // An era is a band before the scale, across the years it spans.
      const z=back-(label.strip%2?34:16);
      lines.push({color:GOLD,alpha:.55,weight:2,points:[[label.span[0],0,z],[label.span[1],0,z]]});
      names.push({id:label.id,variant:'tick',title:label.title,subtitle:'',center:[(label.span[0]+label.span[1])/2,-14,z]});
    }
    else if(label.variant==='lane'){
      const index=lanes.findIndex(lane=>`time:lane:${lane.id}`===label.id);if(index<0)continue;
      const own=[...positions].filter(([id])=>laneOf.get(id)===index).map(([,p])=>p[0]),from=own.length?Math.min(...own):x0;
      names.push({id:label.id,ref:label.ref,variant:'tick',align:'end',title:label.title,subtitle:'',hint:label.hint,center:[from-20,0,zOf(index)],maxWidth:220});
    }
  }
  return {kind:'time',positions,lines,names,laneOf,rest:VOLUME_REST.time,bounds:{x:[x0,x1],z:[back,front]}};
}

/**
 * Place and time as one cube. `geography` is the flat map (space-geography.mjs), `xOfTime` gives a star its place on
 * the scale of time (or null), `timeLabels` the scale's marks, `basemap` the coastlines in the map's units.
 */
export function spaceVolume(geography,{spreadAround=2.6,xOfTime=()=>null,timeBounds=null,timeLabels=[],basemap=[],locale='ru',height=null}={}){
  if(!geography?.positions?.size)return null;
  const [mx0,mx1]=geography.map.x,[my0,my1]=geography.map.y,width=mx1-mx0,H=height??Math.max(320,width*.62);
  const times=[...geography.positions.keys()].map(id=>xOfTime(id)).filter(Number.isFinite);
  const [t0,t1]=timeBounds??(times.length?[Math.min(...times),Math.max(...times)]:[0,1]);
  const lift=t=>Number.isFinite(t)?-(18+(t-t0)/Math.max(1e-9,t1-t0)*H):-6;
  const positions=new Map(),columns=new Map();
  // A place's stars stand round it a little wider than on the flat map: the column has room in the air.
  for(const [id,[x,y]] of geography.positions){
    const own=geography.placeOf(id),[px,py]=own?geography.toXY(own.point):[x,y],spread=own?spreadAround:1;
    const p=[px+(x-px)*spread,lift(xOfTime(id)),py+(y-py)*spread];positions.set(id,p);
    if(own){const c=columns.get(own.place)??{x:px,z:py,top:0};c.top=Math.min(c.top,p[1]);columns.set(own.place,c);}
  }
  const lines=[],names=[];
  // The map on the ground: its frame and its coastlines.
  lines.push({color:RULE,alpha:.4,weight:1.1,points:[[mx0,0,my0],[mx1,0,my0],[mx1,0,my1],[mx0,0,my1],[mx0,0,my0]]});
  for(const line of basemap)if(line.length>1)lines.push({color:NEUTRAL,alpha:.38,weight:1,points:line.map(([x,y])=>[x,0,y])});
  // A place's stars stand in a column over it: a thin stem from the ground to the highest of them.
  for(const c of columns.values())if(c.top<-8)lines.push({color:GOLD,alpha:.32,weight:1,points:[[c.x,0,c.z],[c.x,c.top,c.z]]});
  // The scale of time is a pole at the back of the map; a mark of it is a level drawn round the map at its height.
  const pole=[mx0-24,0,my0-24];lines.push({color:RULE,alpha:.6,weight:1.4,points:[pole,[pole[0],lift(t1)-20,pole[2]]]});
  let levels=0;
  for(const label of timeLabels){
    const t=label.center?.[0];if(!Number.isFinite(t))continue;
    if(label.variant==='axis'||label.variant==='side'){names.push({id:`space:${label.id}`,variant:'tick',align:'start',title:label.title,subtitle:'',center:[pole[0]+6,lift(t),pole[2]]});}
    else if(label.variant==='era'&&label.span){
      const y=lift(label.span[0]);levels++;
      lines.push({color:GOLD,alpha:.18,weight:1,points:[[mx0,y,my0],[mx1,y,my0],[mx1,y,my1],[mx0,y,my1],[mx0,y,my0]]});
      names.push({id:`space:${label.id}`,variant:'tick',align:'start',title:label.title,subtitle:'',center:[mx1+10,y,my0]});
    }
  }
  // The places are named on the ground, at their points.
  for(const label of geography.labels(locale)){
    if(label.variant==='place')names.push({...label,center:[label.center[0],0,label.center[1]]});
    else if(label.variant==='side')names.push({...label,variant:'tick',center:[label.center[0],8,my0]});
  }
  return {kind:'space',positions,lines,names,rest:VOLUME_REST.space,levels,height:H};
}

/**
 * The sphere round the chosen star. `near` is the flat layout of proximity (space-proximity.mjs); `xOfTime` gives each
 * star its time on the lens of time's scale; the height is the time from the chosen star.
 */
export function proximityVolume(near,{xOfTime=()=>null,format=value=>String(value),locale='ru',unmeasuredLabel=null,height=null,words={earlier:'раньше',later:'позже'}}={}){
  if(!near?.positions?.size)return null;
  const H=height??near.radius*.55,t0=xOfTime(near.center);
  const spread=Math.max(1e-9,...[...near.positions.keys()].map(id=>{const t=xOfTime(id);return Number.isFinite(t)&&Number.isFinite(t0)?Math.abs(t-t0):0;}));
  const lift=id=>{const t=xOfTime(id);return Number.isFinite(t)&&Number.isFinite(t0)?-(t-t0)/spread*H:0;};
  const positions=new Map();for(const [id,[x,y]] of near.positions)positions.set(id,[x,id===near.center?0:lift(id),y]);
  const lines=[],names=[];
  for(const label of near.labels(locale,{format,...(unmeasuredLabel?{unmeasuredLabel}:{})})){
    if(label.variant==='ring')lines.push({color:label.tone==='strong'?GOLD:NEUTRAL,alpha:label.tone==='strong'?.45:.34,weight:1,points:circle(label.radius,0)});
    else if(label.variant==='tick'&&label.id.startsWith('near:tick:')){const r=Math.abs(label.center[1]);names.push({...label,center:[0,6,r+8]});}
    else if(label.variant==='tick')names.push({...label,center:[label.center[0],0,label.center[1]]});
  }
  // The axis of time through the chosen star: earlier below, later above.
  if(Number.isFinite(t0)){
    lines.push({color:GOLD,alpha:.55,weight:1.4,points:[[0,H+20,0],[0,-H-20,0]]});
    names.push({id:'near:later',variant:'tick',align:'start',title:words.later,subtitle:'',center:[10,-H-20,0]},{id:'near:earlier',variant:'tick',align:'start',title:words.earlier,subtitle:'',center:[10,H+20,0]});
  }
  return {kind:'proximity',positions,lines,names,rest:VOLUME_REST.proximity};
}
