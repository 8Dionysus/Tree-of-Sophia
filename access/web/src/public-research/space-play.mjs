// The play of knowing in a stand: nothing is scored, and nothing is hidden
// from the reader; what grows is the reader's own understanding, set among
// the readings it meets.
//
// · Your own reading first: at a step of a branch or route, and at a riddle
//   (a question the text itself poses), the reader may write one line before
//   the readings open; then they set their star beside the reading they stand
//   nearest and see what follows: what that reading rests on, and what
//   disputes it.
// · The edges of what is known: from a chosen star, threads lead into the dark
//   toward the stars the space does not show yet; following one brings that
//   star into the space.
//
// These functions only read the prepared atlas and the reader's own records;
// they never write a reading, a relation or a source.

const DISPUTE=new Set(['contrasts','questions']);
const READING=new Set(['reading','comparison']);
// Readings close to a question first: what interprets or develops it, then what compares or echoes it.
const ORDER={interprets:0,develops:1,supports:2,questions:3,contrasts:3,compares:4,echoes:5,relates:6,translates:7,contains:8};

const ends=(edge,id)=>edge.from===id?edge.to:edge.to===id?edge.from:null;

/**
 * The readings around a target: the stars linked to it whose step of
 * interpretation is a reading or a comparison, nearest kinds first, each once.
 */
export function readingsAround(targetId,{edges=[],rungOf=()=>null,limit=6}={}){
  const seen=new Map();
  for(const edge of edges){
    const other=ends(edge,targetId);if(!other||other===targetId||!READING.has(rungOf(other)))continue;
    const rank=ORDER[edge.kind]??9;if(!seen.has(other)||seen.get(other).rank>rank)seen.set(other,{id:other,kind:edge.kind,rank});
  }
  return [...seen.values()].sort((a,b)=>a.rank-b.rank).slice(0,limit).map(({id,kind})=>({id,kind}));
}

/** What a reading rests on: the sources linked to it, each once. */
export function restsOn(id,{edges=[],rungOf=()=>null,limit=4}={}){
  const out=[];
  for(const edge of edges){const other=ends(edge,id);if(other&&other!==id&&rungOf(other)==='source'&&!out.includes(other))out.push(other);}
  return out.slice(0,limit);
}

/** What disputes a reading: the stars that answer it otherwise or put it in question, each once. */
export function disputes(id,{edges=[],limit=4}={}){
  const out=[];
  for(const edge of edges){if(!DISPUTE.has(edge.kind))continue;const other=ends(edge,id);if(other&&other!==id&&!out.includes(other))out.push(other);}
  return out.slice(0,limit);
}

/** A reader's line as the title of their record: its first line, cut at a word near 90 letters. */
export function ownTitle(text){
  const line=String(text??'').replace(/\s+/g,' ').trim();
  if(line.length<=90)return line;
  const cut=line.slice(0,90),space=cut.lastIndexOf(' ');
  return `${(space>50?cut.slice(0,space):cut).replace(/[\s,.;:—–-]+$/u,'')}…`;
}

/** Where a reader's star stands beside another's: a little up and to the side, never on it. */
export function besidePosition(position,index=0){
  const [x=0,y=0,z=0]=Array.isArray(position)?position:[];
  // On a ring around the other star, first up and to the right, each next one further round.
  const turn=-0.65+index*1.9,dx=Math.cos(turn)*58,dy=Math.sin(turn)*44;
  return [Math.round(x+dx),Math.round(y+dy),z];
}

/**
 * The threads from a chosen star into the dark: its links to stars the space
 * does not show, each with the direction it leaves in (on the map's own plane)
 * and the kind of link. Stars without a known place give no thread.
 */
export function threadsIntoDark(id,{edges=[],shown=new Set(),positionOf=()=>null,limit=6}={}){
  const from=positionOf(id);if(!from)return [];
  const out=new Map();
  for(const edge of edges){
    const other=ends(edge,id);if(!other||other===id||shown.has(other)||out.has(other))continue;
    const to=positionOf(other);if(!to)continue;
    let dx=to[0]-from[0],dy=to[1]-from[1];const length=Math.hypot(dx,dy);
    // A star at the same place still leaves in a direction of its own.
    if(length<1){const angle=(out.size*2.4)%(Math.PI*2);dx=Math.cos(angle);dy=Math.sin(angle);}else{dx/=length;dy/=length;}
    out.set(other,{id:other,kind:edge.kind,direction:[dx,dy]});
  }
  return [...out.values()].sort((a,b)=>(ORDER[a.kind]??9)-(ORDER[b.kind]??9)).slice(0,limit);
}

/** The stars of a route's steps that the space shows, in the order the route walks them, each once. */
export function routeStars(steps=[],{graphIdOf=id=>id,shown=new Set()}={}){
  const out=[];
  for(const step of steps){const raw=step?.nodeId??step?.graphNodeId;if(!raw)continue;const id=graphIdOf(raw);if(shown.has(id)&&!out.includes(id))out.push(id);}
  return out;
}
