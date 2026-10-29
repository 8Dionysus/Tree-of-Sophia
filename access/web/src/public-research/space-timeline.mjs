// The lens "Time" places only what the data already dates or orders.
// Antiquity: a move stands within the span its bonsai's chronology gives the
// writing of its passage or work — a date, an interval, or a phase of a life
// the writing is dated "during", each with its source; a writing whose date
// the chronology states as unknown stands outside the scale. Moves sharing
// one span in a branch spread over it in the order of the steps; each branch
// is one row. Years are astronomical (1 BCE = 0); BCE/CE is the reader's
// wording only. Zarathustra: a star stands at the first chapter of the book it
// rests on; stars grounded only in other texts stand outside the scale; each
// part names the years its bonsai dates its writing to. The layout is display
// only: positions are never written into the graph.

import {readChronology} from '../../../../bonsai/lib/chronology.mjs';
import {centuryOf,centuryStart,formatDating,formatYears} from '../../../../bonsai/lib/time.mjs';

export {formatDating,formatYears};
const lang=locale=>String(locale||'ru').startsWith('en')?'en':'ru';
const pick=(value,locale)=>typeof value==='string'?value:value?.[lang(locale)]??value?.en??value?.ru??'';
const ROMAN={I:1,V:5,X:10,L:50,C:100};
const ROW=13;

/** The value of a Roman numeral (I–C range), or NaN. */
export function romanValue(text){
  const s=String(text??'').toUpperCase();if(!/^[IVXLC]+$/.test(s))return NaN;
  let total=0;for(let i=0;i<s.length;i++){const value=ROMAN[s[i]],next=ROMAN[s[i+1]]??0;total+=value<next?-value:value;}
  return total;
}

/** Where a reference lies in Zarathustra: its part and the chapter number of the edition's table. */
export function bookPlace(reference){
  const locator=pick(reference?.locator,'en'),work=pick(reference?.work,'en');
  if(!/zarathustra/i.test(work))return null;
  const part=romanValue(locator.match(/^(?:Part\s+)?([IV]+)\s*·/)?.[1]);
  if(/prologue/i.test(locator))return {part:Number.isFinite(part)?part:1,chapter:0};
  const chapter=romanValue(locator.match(/\b([IVXLC]+)\.\s/)?.[1]);
  return Number.isFinite(chapter)?{part:Number.isFinite(part)?part:null,chapter}:null;
}

const centuryLabel=(start,locale)=>{
  const {n,era}=centuryOf(start),roman=['I','II','III','IV','V','VI','VII','VIII','IX','X'][n-1]??String(n);
  return lang(locale)==='en'?`${n}${n===1?'st':n===2?'nd':n===3?'rd':'th'} c.`:`${roman} в.`;
};

/** Stack stars that share a place on one row, so that none hides another and x stays exact. */
function placeRow(ids,xOf,top){
  const stacks=new Map();
  for(const id of ids){const x=Math.round(xOf(id));if(!stacks.has(x))stacks.set(x,[]);stacks.get(x).push(id);}
  const depth=Math.max(1,...[...stacks.values()].map(list=>list.length)),height=Math.max(34,depth*ROW+18),center=top+height/2,positions=new Map();
  for(const [x,list] of stacks)list.forEach((id,index)=>positions.set(id,[x,center+(index-(list.length-1)/2)*ROW,0]));
  return {positions,center,height};
}

/**
 * Keep one row of stars on one line: each star keeps its order and stays as
 * near its value as a minimal gap between neighbours allows.
 */
export function dodgeRow(items,gap){
  const groups=[],first=group=>group.sum/group.items.length-(group.items.length-1)*gap/2;
  for(const item of [...items].sort((a,b)=>a.x-b.x||(a.order??0)-(b.order??0))){
    groups.push({items:[item],sum:item.x});
    while(groups.length>1){
      const b=groups.at(-1),a=groups.at(-2);
      if(first(a)+a.items.length*gap<=first(b))break;
      groups.splice(-2,2,{items:[...a.items,...b.items],sum:a.sum+b.sum});
    }
  }
  const out=new Map();
  for(const group of groups){const start=first(group);group.items.forEach((item,index)=>out.set(item.id,start+index*gap));}
  return out;
}

/**
 * Stars that still share a place on one row after keeping to their spans stand
 * in a short column there, alternately above and below the row's line.
 */
export function stackColumns(xs,minDx,dy){
  const out=new Map(),placed=[];
  for(const [id,x] of [...xs.entries()].sort((a,b)=>a[1]-b[1])){
    const level=[0,-1,1,-2,2,-3,3].find(candidate=>!placed.some(other=>other.level===candidate&&Math.abs(other.x-x)<minDx))??0;
    placed.push({x,level});out.set(id,level*dy);
  }
  return out;
}

// The scale's strip above the map, in pixels from the map's top edge to each row's top.
const YEAR_ROWS={side:22,axis:46,era1:72,era0:98},BOOK_ROWS={side:22,axis:46,era0:72,era1:98};
const LANE=36,GAP=12,STACK=7;

/**
 * Antiquity on a scale of years: one row per branch, each move within the span
 * its bonsai's chronology gives its writing. A move walked by two branches
 * stands in the first; a writing the chronology dates as unknown stands with
 * the other undated ones outside the scale.
 */
export function antiquityTimeline({routes=[],entries=[],chronology=null}={}){
  const byId=new Map(entries.map(entry=>[entry.legacyId??entry.id,entry])),reader=readChronology(chronology);
  // Each author's life as the chronology dates it; it spans an era's bracket.
  const lives=new Map();
  for(const entry of entries)if(entry.kind==='author'){const id=entry.legacyId??entry.id,situation=reader.lifeOf(id),span=situation?reader.span(situation):null;if(span)lives.set(id,{situation,span});}
  const timeOf=new Map();
  for(const route of routes)for(const step of route.steps??[]){
    const id=step.nodeId??step.graphNodeId;if(!id||timeOf.has(id))continue;
    const passageId=String(id).replace(/^antiquity:reading:/,''),passage=byId.get(passageId);if(!passage)continue;
    const workId=passage.workId??null,authorId=passage.author??byId.get(workId)?.author??null,composition=reader.compositionOf({passageId,workId});
    timeOf.set(id,{passageId,workId,authorId,life:lives.get(authorId)?.situation??null,composition,span:composition?reader.span(composition):null,period:pick(passage.period,'en')});
  }
  if(!timeOf.size||!lives.size)return null;
  const spanOf=id=>timeOf.get(id)?.span??null,undated=new Set([...timeOf].filter(([,time])=>!time.span).map(([id])=>id));
  // The scale spans every life and every dated writing, so that no writing falls off it (a corpus of many hands has no life).
  const all=[...[...lives.values()].map(life=>life.span),...[...timeOf.values()].map(time=>time.span).filter(Boolean)];
  const min=centuryStart(Math.floor(Math.min(...all.map(span=>span.from)))),max=centuryStart(Math.ceil(Math.max(...all.map(span=>span.to)))-1)+100;
  // The column past the scale is "date unknown" where every writing in it is dated so, else only "off the scale".
  const unknownOnly=[...undated].every(id=>reader.situation(timeOf.get(id)?.composition)?.dating?.kind==='unknown-date');
  const width=1000,xOfYear=year=>(year-min)/(max-min)*width-width/2,outsideX=width/2+130;
  const positions=new Map(),lanes=[],yearOf=new Map();
  for(const route of routes){
    const ids=[...new Set((route.steps??[]).map(step=>step.nodeId??step.graphNodeId))].filter(id=>timeOf.has(id)&&!positions.has(id));
    if(!ids.length)continue;
    const dated=ids.filter(id=>spanOf(id));
    // Moves sharing one span in a branch spread over it in the order of the steps.
    const bySpan=new Map();for(const id of dated){const span=spanOf(id),key=`${span.from}:${span.to}`;if(!bySpan.has(key))bySpan.set(key,[]);bySpan.get(key).push(id);}
    for(const list of bySpan.values())list.forEach((id,index)=>{const span=spanOf(id);yearOf.set(id,span.from+(index+.5)/list.length*(span.to-span.from));});
    // Neighbours keep a gap, but no star leaves its span to make room; stars that still meet share a short column.
    const dodged=dodgeRow(dated.map((id,order)=>({id,x:xOfYear(yearOf.get(id)),order})),GAP);
    const xs=new Map(dated.map(id=>{const span=spanOf(id);return [id,Math.min(xOfYear(span.to),Math.max(xOfYear(span.from),dodged.get(id)))];}));
    ids.filter(id=>!spanOf(id)).forEach((id,index)=>xs.set(id,outsideX+index*GAP));
    const y=lanes.length*LANE,dys=stackColumns(xs,GAP/2,STACK);
    for(const [id,x] of xs)positions.set(id,[x,y+dys.get(id),0]);
    const inside=dated.map(id=>xs.get(id));
    lanes.push({id:route.id,title:route.title,y,from:Math.min(...(inside.length?inside:[...xs.values()])),to:Math.max(...(inside.length?inside:[...xs.values()]))});
  }
  // Rows are centred on the scale's origin.
  const shift=-(lanes.length-1)*LANE/2;for(const position of positions.values())position[1]+=shift;for(const lane of lanes)lane.y+=shift;
  const eras=entries.filter(entry=>entry.kind==='era'),livesOf=period=>[...lives.entries()].filter(([id])=>pick(byId.get(id)?.period,'en')===period).map(([,life])=>life.span);
  return {
    kind:'years',positions,lanes,domain:{min,max},rows:YEAR_ROWS,strip:YEAR_ROWS.era0+4,bounds:[xOfYear(min),undated.size?outsideX:xOfYear(max)],
    xOf:id=>yearOf.has(id)?xOfYear(yearOf.get(id)):undated.has(id)?outsideX:null,xOfYear,
    timeOf:id=>timeOf.get(id)??null,spanOf,isOutside:id=>undated.has(id),reader,
    lifeSpan:authorId=>lives.get(authorId)?.span??null,
    labels(locale,{laneWidth=null}={}){
      const en=lang(locale)==='en',out=[];
      // A century begins at its year 100k + 1; the first year of the era (1 CE) is the strong rule between BCE and CE.
      for(let year=min;year<=max;year+=100)out.push({id:`time:rule:${year}`,variant:'rule',title:'',subtitle:'',center:[xOfYear(year),0,0],...(year===1?{tone:'strong'}:{})});
      for(let start=min;start<max;start+=100)out.push({id:`time:c${start}`,variant:'axis',strip:YEAR_ROWS.axis,title:centuryLabel(start,locale),subtitle:'',center:[xOfYear(start+50),0,0]});
      // The eras' boundary is named once, on its two sides.
      if(min<1&&max>1)out.push({id:'time:bce',variant:'side',align:'end',strip:YEAR_ROWS.side,title:en?'BCE':'до н. э.',subtitle:'',center:[xOfYear(1),0,0]},{id:'time:ce',variant:'side',align:'start',strip:YEAR_ROWS.side,title:en?'CE':'н. э.',subtitle:'',center:[xOfYear(1),0,0]});
      // An era spans the lives of its authors in the collection; overlapping eras take two rows.
      eras.map(era=>({era,spans:livesOf(pick(era.period,'en'))})).filter(item=>item.spans.length).forEach(({era,spans},index)=>{
        const span=[xOfYear(Math.min(...spans.map(item=>item.from))),xOfYear(Math.max(...spans.map(item=>item.to)))];
        out.push({id:`time:${era.legacyId??era.id}`,variant:'era',strip:index%2?YEAR_ROWS.era1:YEAR_ROWS.era0,span,title:pick(era.title,locale),subtitle:'',center:[(span[0]+span[1])/2,0,0]});
      });
      // Writings the scale cannot place stand past a strong rule, named on the line of centuries.
      if(undated.size){
        out.push({id:'time:rule:undated',variant:'rule',tone:'strong',title:'',subtitle:'',center:[(xOfYear(max)+outsideX)/2,0,0]});
        out.push({id:'time:undated',variant:'side',strip:YEAR_ROWS.axis,title:unknownOnly?(en?'date unknown':'дата неизвестна'):(en?'off the scale':'вне шкалы'),subtitle:'',center:[outsideX,0,0]});
      }
      for(const lane of lanes){
        out.push({id:`time:thread:${lane.id}`,variant:'thread',title:'',subtitle:'',span:[lane.from,lane.to],center:[(lane.from+lane.to)/2,lane.y,0]});
        out.push({id:`time:lane:${lane.id}`,variant:'lane',ref:lane.id,title:pick(lane.title,locale),subtitle:'',hint:en?`Open the branch “${pick(lane.title,locale)}”`:`Открыть ветвь «${pick(lane.title,locale)}»`,center:[lane.from,lane.y,0],...(laneWidth?{maxWidth:laneWidth}:{})});
      }
      return out;
    },
  };
}

/**
 * Zarathustra on the order of the book: one row per circle of the map, each
 * star at the first chapter it rests on. Stars grounded only in other texts
 * stand together outside the scale.
 */
export function zarathustraTimeline({materials=[],references={},clusters=[],chronology=null}={}){
  // The years each part was written, as the bonsai's chronology dates it.
  const reader=chronology?readChronology(chronology):null;
  const partYears=(part,locale)=>{const id=(chronology?.book?.parts??[]).find(item=>item.part===part)?.writing,span=id?reader.span(id):null;return span?formatYears({...span,certainty:'exact'},locale):'';};
  const dated=(chronology?.book?.parts??[]).some(item=>item.writing&&reader?.span(item.writing));
  const placeOf=new Map(),outside=new Set(),groundsOf=new Map();
  for(const material of materials){
    const refs=(material.inquiry?.grounds??[]).map(ground=>ground?.ref).filter(ref=>references?.[ref]);
    if(!refs.length)continue;
    groundsOf.set(material.id,refs);
    const places=refs.map(ref=>({ref,...bookPlace(references[ref])})).filter(place=>Number.isFinite(place.chapter));
    if(places.length)placeOf.set(material.id,places.reduce((a,b)=>b.chapter<a.chapter?b:a));else outside.add(material.id);
  }
  // A star rests first on the earliest chapter it cites, else on the first text it cites outside the book.
  const firstGround=id=>placeOf.get(id)?.ref??groundsOf.get(id)?.[0]??null;
  if(!placeOf.size)return null;
  const chapters=[...placeOf.values()].map(place=>place.chapter),maxChapter=Math.max(...chapters,1);
  const width=880,xOfChapter=chapter=>chapter/maxChapter*width-width/2,outsideX=width/2+130;
  const xOf=id=>placeOf.has(id)?xOfChapter(placeOf.get(id).chapter):outside.has(id)?outsideX:null;
  const byCluster=new Map(clusters.map(cluster=>[cluster.id,[]]));
  for(const material of materials)if(xOf(material.id)!==null){const key=byCluster.has(material.cluster)?material.cluster:'__other';if(!byCluster.has(key))byCluster.set(key,[]);byCluster.get(key).push(material.id);}
  const positions=new Map(),lanes=[];let top=0;
  for(const [clusterId,ids] of byCluster){
    if(!ids.length)continue;
    const row=placeRow(ids,xOf,top);for(const [id,position] of row.positions)positions.set(id,position);
    const cluster=clusters.find(item=>item.id===clusterId);lanes.push({id:clusterId,title:cluster?.title??'',y:row.center});top+=row.height;
  }
  const shift=-top/2;for(const position of positions.values())position[1]+=shift;for(const lane of lanes)lane.y+=shift;
  const parts=new Map();
  for(const place of placeOf.values())if(place.part){if(!parts.has(place.part))parts.set(place.part,[]);parts.get(place.part).push(place.chapter);}
  const marked=[...new Set(chapters)].sort((a,b)=>a-b),ordered=[...parts].sort((a,b)=>a[0]-b[0]);
  return {
    // Parts named with their years take two rows of the strip, alternately, so that each name keeps its room.
    kind:'book',positions,lanes,domain:{min:0,max:maxChapter},rows:BOOK_ROWS,strip:(dated?BOOK_ROWS.era1:BOOK_ROWS.era0)+4,bounds:[xOfChapter(0),outside.size?outsideX:xOfChapter(maxChapter)],
    xOf,placeOf:id=>{const place=placeOf.get(id);return place?{part:place.part,chapter:place.chapter}:null;},isOutside:id=>outside.has(id),
    groundsOf:id=>groundsOf.get(id)??[],firstGround,outsideWorkOf:id=>outside.has(id)?pick(references[firstGround(id)]?.work,'en')||null:null,
    labels(locale,{laneWidth=null}={}){
      const en=lang(locale)==='en',out=[];
      for(const chapter of marked)out.push({id:`time:ch${chapter}`,variant:'axis',strip:BOOK_ROWS.axis,title:chapter===0?(en?'Prologue':'Пролог'):toRoman(chapter),subtitle:'',center:[xOfChapter(chapter),0,0]});
      // A part spans the chapters the map cites from it; a rule stands between parts and before the texts outside the book,
      // whose column is named on the line of chapters, since it has none.
      ordered.forEach(([part,list],index)=>{
        const span=[xOfChapter(Math.min(...list)),xOfChapter(Math.max(...list))];
        const years=partYears(part,locale);
        out.push({id:`time:part${part}`,variant:'era',strip:dated&&index%2?BOOK_ROWS.era1:BOOK_ROWS.era0,span,title:`${en?`Part ${toRoman(part)}`:`Часть ${toRoman(part)}`}${years?` · ${years}`:''}`,subtitle:'',center:[(span[0]+span[1])/2,0,0]});
        const next=ordered[index+1];if(next)out.push({id:`time:rule:part${part}`,variant:'rule',title:'',subtitle:'',center:[xOfChapter((Math.max(...list)+Math.min(...next[1]))/2),0,0]});
      });
      if(outside.size){
        out.push({id:'time:rule:outside',variant:'rule',tone:'strong',title:'',subtitle:'',center:[(xOfChapter(maxChapter)+outsideX)/2,0,0]});
        out.push({id:'time:outside',variant:'side',strip:BOOK_ROWS.axis,title:en?'outside the book':'вне книги',subtitle:'',center:[outsideX,0,0]});
      }
      for(const lane of lanes)out.push({id:`time:lane:${lane.id}`,variant:'lane',title:pick(lane.title,locale),subtitle:'',center:[0,lane.y,0],...(laneWidth?{maxWidth:laneWidth}:{})});
      return out;
    },
  };
}

/** A whole number in Roman numerals: 80 → "LXXX". */
export function toRoman(value){
  let n=Math.max(0,Math.floor(value)),out='';
  for(const [symbol,amount] of [['C',100],['XC',90],['L',50],['XL',40],['X',10],['IX',9],['V',5],['IV',4],['I',1]])while(n>=amount){out+=symbol;n-=amount;}
  return out;
}

/**
 * An open branch or route on the same scale: its steps keep their place in
 * time and read top to bottom in their own order.
 */
export function stepTimeline(stepIds,xOf,{row=46,outsideX=null}={}){
  const positions=new Map(),placed=stepIds.filter(id=>xOf(id)!==null||outsideX!==null);
  placed.forEach((id,index)=>positions.set(id,[xOf(id)??outsideX,(index-(placed.length-1)/2)*row,0]));
  return positions;
}

/**
 * A concept's trajectory on a scale: the stars that carry it, ordered by their
 * place on the scale, each joined to the next. It shows where the concept
 * recurs and claims no influence between its stations. Stars the scale cannot
 * place stay out of the line.
 */
export function trajectory(ids,xOf){
  const placed=[...new Set(ids)].filter(id=>Number.isFinite(xOf(id))).sort((a,b)=>xOf(a)-xOf(b)||String(a).localeCompare(String(b)));
  return {ids:placed,links:placed.slice(1).map((id,index)=>[placed[index],id])};
}
