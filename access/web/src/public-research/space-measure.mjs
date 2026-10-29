import {resolveLocale,copyLocale,localeText,localeValue} from './locale.mjs';
// Measure and completeness: what a stand's data dates, places and links, and
// what it does not. Counts come from the bonsai and the routes as they stand;
// nothing here is estimated. A branch's "returns" count the steps that go back
// in time (Antiquity) or in the order of the book (Zarathustra): an argument
// that turns to an earlier text, which the branch's order shows and the scale
// measures.

const lang=resolveLocale;
const pick=localeValue;
const count=(map,key)=>map.set(key,(map.get(key)??0)+1);

/** How many times a sequence of values steps back below its previous value. */
export function returnsIn(values){
  let returns=0;const known=values.filter(Number.isFinite);
  for(let i=1;i<known.length;i++)if(known[i]<known[i-1]-1e-9)returns++;
  return returns;
}

/** The kind of a dating as the reader meets it: its Tree kind, and for a relative one what it is relative to. */
export function datingKind(situation,reader){
  const dating=reader?.situation(situation)?.dating;if(!dating)return 'none';
  if(dating.kind!=='relative-order')return dating.kind;
  const ref=dating.relative?.anchor_ref??'',anchor=reader.situation(ref);
  // "During" a whole life, or the years of activity that are all a source dates, says the least a dating can say.
  return /\.lifetime$/.test(ref)?'relative-life':/\.activity$/.test(ref)?'relative-activity':anchor?'relative-phase':'relative-order';
}

/** Antiquity: every move of every branch, by what its bonsai dates and places. */
export function measureAntiquity({routes=[],entries=[],timeline=null,placeOf=()=>null}={}){
  if(!timeline)return null;
  const reader=timeline.reader,byId=new Map(entries.map(entry=>[entry.legacyId??entry.id,entry]));
  const moves=[...timeline.positions.keys()],kinds=new Map(),certainty=new Map(),footing=new Map(),bases=new Map(),eras=new Map(),authors=new Map(),concepts=new Map();
  const lifeOnly=new Map(),placeless=new Map(),eraYears=new Map();
  for(const id of moves){
    const time=timeline.timeOf(id),kind=datingKind(time?.composition,reader),situation=reader.situation(time?.composition);
    // A dating within a phase or a life is counted as firm as that span, not firmer.
    count(kinds,kind);count(certainty,timeline.spanOf(id)?.certainty??situation?.dating?.certainty??'none');count(footing,situation?.footing??'none');
    const place=placeOf(id);count(bases,place?.basis??'unknown');
    count(eras,time?.period??'');count(authors,time?.authorId??'');
    const span=timeline.spanOf(id);if(span){const list=eraYears.get(time?.period??'')??[];list.push((span.from+span.to)/2);eraYears.set(time?.period??'',list);}
    for(const concept of byId.get(time?.passageId)?.concepts??[])count(concepts,concept);
    if((kind==='relative-life'||kind==='relative-activity')&&time?.workId)count(lifeOnly,time.workId);
    if(!place&&time?.workId)count(placeless,time.workId);
  }
  const middle=id=>{const span=timeline.spanOf(id);return span?(span.from+span.to)/2:NaN;};
  const branches=routes.map(route=>{
    const ids=(route.steps??[]).map(step=>step.nodeId??step.graphNodeId).filter(id=>timeline.positions.has(id)),mids=ids.map(middle).filter(Number.isFinite);
    const places=new Set(ids.map(id=>placeOf(id)?.place).filter(Boolean));
    return {id:route.id,title:route.title,moves:ids.length,first:mids.length?Math.min(...mids):null,last:mids.length?Math.max(...mids):null,returns:returnsIn(ids.map(middle)),places:places.size};
  });
  const sorted=map=>[...map.entries()].sort((a,b)=>b[1]-a[1]||String(a[0]).localeCompare(String(b[0])));
  return {
    kind:'years',total:moves.length,
    dating:{kinds:Object.fromEntries(kinds),certainty:Object.fromEntries(certainty),footing:Object.fromEntries(footing)},
    places:{bases:Object.fromEntries(bases)},
    // Eras in the order of time: by the middle year of the moves they hold.
    eras:[...eras.entries()].map(([period,n])=>({period,n,title:entries.find(entry=>entry.kind==='era'&&pick(entry.period,'en')===period)?.title??period,middle:(eraYears.get(period)??[]).reduce((sum,value,_,list)=>sum+value/list.length,0)})).sort((a,b)=>a.middle-b.middle),
    authors:sorted(authors).map(([id,n])=>({id,n,title:byId.get(id)?.title??id})),
    concepts:sorted(concepts).map(([id,n])=>({id,n,title:byId.get(id)?.title??id})),
    branches,
    gaps:{lifeOnly:sorted(lifeOnly).map(([id,n])=>({id,n,title:byId.get(id)?.title??id})),placeless:sorted(placeless).map(([id,n])=>({id,n,title:byId.get(id)?.title??id}))},
  };
}

/** Zarathustra: the book's chapters the map rests on, the texts outside it, and how its routes walk the book. */
export function measureZarathustra({routes=[],timeline=null,book=null,placeOf=()=>null,materials=[]}={}){
  if(!timeline)return null;
  const stars=[...timeline.positions.keys()],inBook=stars.filter(id=>!timeline.isOutside(id)),cited=new Set(inBook.map(id=>timeline.placeOf(id)?.chapter).filter(Number.isFinite));
  const parts=(book?.parts??[]).map(part=>{const [from,to]=part.chapters;let n=0;for(let c=from;c<=to;c++)if(cited.has(c))n++;return {part:part.part,cited:n,total:to-from+1,writing:part.writing??null};});
  const total=(book?.parts??[]).reduce((sum,part)=>sum+part.chapters[1]-part.chapters[0]+1,0);
  const outsideTexts=new Map(),outsideRefs=new Map(),bases=new Map();
  const byId=new Map(materials.map(material=>[material.id,material]));
  for(const id of stars){const place=placeOf(id);count(bases,place?.basis??'unknown');}
  for(const id of stars.filter(id=>timeline.isOutside(id))){const work=timeline.outsideWorkOf?.(id)??null;count(outsideTexts,work??'');if(!outsideRefs.has(work??''))outsideRefs.set(work??'',timeline.firstGround?.(id)??null);}
  const chapterOf=id=>timeline.placeOf(id)?.chapter;
  const routeRows=routes.map(route=>{
    const ids=(route.steps??[]).map(step=>step.nodeId??step.graphNodeId).filter(id=>timeline.positions.has(id)),chapters=ids.map(chapterOf).filter(Number.isFinite);
    return {id:route.id,title:route.title,steps:ids.length,first:chapters.length?Math.min(...chapters):null,last:chapters.length?Math.max(...chapters):null,returns:returnsIn(ids.map(chapterOf)),outside:ids.filter(id=>timeline.isOutside(id)).length};
  });
  return {kind:'book',total:stars.length,inBook:inBook.length,outside:stars.length-inBook.length,chapters:{cited:cited.size,total},parts,places:{bases:Object.fromEntries(bases)},
    outsideTexts:[...outsideTexts.entries()].sort((a,b)=>b[1]-a[1]).map(([work,n])=>({work,n,ref:outsideRefs.get(work)??null})),routes:routeRows,titleOf:id=>byId.get(id)?.title??id};
}
