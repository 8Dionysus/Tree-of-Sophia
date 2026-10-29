// The lens "Time" places only what the data already dates or orders.
// Antiquity: a move stands within its author's life, because the collection
// records authors' life dates and not dates of composition; one author's moves
// in a branch spread over that life in the order of the steps, and each branch
// is one row. Zarathustra: a star stands at the first chapter of the
// book it rests on; stars grounded only in other texts stand outside the
// scale, since the data gives those texts no dates. The layout is display
// only: positions are never written into the graph.

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

/**
 * Years of an authored life-date line, in its English form: "c. 429–347 BCE",
 * "45–47–c. 120 CE", "c. 1 BCE–65 CE", "mid-90s to mid-50s BCE",
 * "5th–4th c. BCE", "3rd century CE". Years before the era are negative.
 * The precision says how much the line itself claims.
 */
export function parseLifeYears(text){
  const raw=String(text??'').split('·')[0].trim();if(!raw)return null;
  const bceAt=raw.search(/\bBCE\b/);
  const sign=index=>bceAt>=0&&index<bceAt?-1:1;
  const ordinals=[...raw.matchAll(/(\d+)(?:st|nd|rd|th)\b/g)];
  if(ordinals.length&&/\bc\.|century/i.test(raw)){
    const centuries=ordinals.map(match=>({n:Number(match[1]),sign:sign(match.index)}));
    const spans=centuries.map(({n,sign})=>sign<0?[-n*100,-(n-1)*100-1]:[(n-1)*100+1,n*100]);
    return {start:Math.min(...spans.map(span=>span[0])),end:Math.max(...spans.map(span=>span[1])),precision:'century'};
  }
  const tokens=[...raw.matchAll(/(\d+)(s?)\b/g)].map(match=>({value:Number(match[1])+(match[2]?5:0),decade:!!match[2],sign:sign(match.index)}));
  if(!tokens.length)return null;
  const years=tokens.map(token=>token.value*token.sign);
  const precision=tokens.some(token=>token.decade)?'decade':/\bc\.|\bca\.|~/.test(raw)?'approx':'year';
  return {start:Math.min(...years),end:Math.max(...years),precision};
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
  const n=start<0?-start/100:start/100+1,roman=['I','II','III','IV','V','VI','VII','VIII'][n-1]??String(n);
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

// The scale's strip above the map, in pixels from the map's top edge to each row's top.
const YEAR_ROWS={side:22,axis:46,era1:72,era0:98},BOOK_ROWS={axis:22,era0:48};
const LANE=36,GAP=12;

/**
 * Antiquity on a scale of years: one row per branch, each move within its
 * author's life. A move walked by two branches stands in the first.
 */
export function antiquityTimeline({routes=[],entries=[]}={}){
  const byId=new Map(entries.map(entry=>[entry.legacyId??entry.id,entry]));
  const lives=new Map();
  for(const entry of entries)if(entry.kind==='author'){const years=parseLifeYears(pick(entry.meta,'en'));if(years)lives.set(entry.legacyId??entry.id,years);}
  const timeOf=new Map();
  for(const route of routes)for(const step of route.steps??[]){
    const id=step.nodeId??step.graphNodeId;if(!id||timeOf.has(id))continue;
    const passage=byId.get(String(id).replace(/^antiquity:reading:/,'')),authorId=passage?.author??byId.get(passage?.workId)?.author,years=lives.get(authorId);
    if(years)timeOf.set(id,{...years,authorId,period:pick(passage.period,'en')});
  }
  if(!timeOf.size)return null;
  const all=[...lives.values()],min=Math.floor(Math.min(...all.map(y=>y.start))/100)*100,max=Math.ceil(Math.max(...all.map(y=>y.end))/100)*100;
  const width=1000,xOfYear=year=>(year-min)/(max-min)*width-width/2;
  const positions=new Map(),lanes=[],yearOf=new Map();
  for(const route of routes){
    const ids=[...new Set((route.steps??[]).map(step=>step.nodeId??step.graphNodeId))].filter(id=>timeOf.has(id)&&!positions.has(id));
    if(!ids.length)continue;
    // The data dates the author, not the move: one author's moves share that life in the order of the steps.
    const byAuthor=new Map();for(const id of ids){const key=timeOf.get(id).authorId;if(!byAuthor.has(key))byAuthor.set(key,[]);byAuthor.get(key).push(id);}
    for(const list of byAuthor.values())list.forEach((id,index)=>{const life=timeOf.get(id);yearOf.set(id,life.start+(index+.5)/list.length*(life.end-life.start));});
    const y=lanes.length*LANE,xs=dodgeRow(ids.map((id,order)=>({id,x:xOfYear(yearOf.get(id)),order})),GAP);
    for(const [id,x] of xs)positions.set(id,[x,y,0]);
    lanes.push({id:route.id,title:route.title,y,from:Math.min(...xs.values()),to:Math.max(...xs.values())});
  }
  // Rows are centred on the scale's origin.
  const shift=-(lanes.length-1)*LANE/2;for(const position of positions.values())position[1]+=shift;for(const lane of lanes)lane.y+=shift;
  const eras=entries.filter(entry=>entry.kind==='era'),authorsOf=period=>[...lives.entries()].filter(([id])=>pick(byId.get(id)?.period,'en')===period).map(([,years])=>years);
  return {
    kind:'years',positions,lanes,domain:{min,max},rows:YEAR_ROWS,strip:YEAR_ROWS.era0+4,bounds:[xOfYear(min),xOfYear(max)],
    xOf:id=>yearOf.has(id)?xOfYear(yearOf.get(id)):null,xOfYear,
    lifeOf:id=>timeOf.get(id)??null,
    labels(locale,{laneWidth=null}={}){
      const en=lang(locale)==='en',out=[];
      for(let year=min;year<=max;year+=100)out.push({id:`time:rule:${year}`,variant:'rule',title:'',subtitle:'',center:[xOfYear(year),0,0],...(year===0?{tone:'strong'}:{})});
      for(let start=min;start<max;start+=100)out.push({id:`time:c${start}`,variant:'axis',strip:YEAR_ROWS.axis,title:centuryLabel(start,locale),subtitle:'',center:[xOfYear(start+50),0,0]});
      // The eras' boundary is named once, on its two sides.
      if(min<0&&max>0)out.push({id:'time:bce',variant:'side',align:'end',strip:YEAR_ROWS.side,title:en?'BCE':'до н. э.',subtitle:'',center:[xOfYear(0),0,0]},{id:'time:ce',variant:'side',align:'start',strip:YEAR_ROWS.side,title:en?'CE':'н. э.',subtitle:'',center:[xOfYear(0),0,0]});
      // An era spans the lives of its authors in the collection; overlapping eras take two rows.
      eras.map(era=>({era,years:authorsOf(pick(era.period,'en'))})).filter(item=>item.years.length).forEach(({era,years},index)=>{
        const span=[xOfYear(Math.min(...years.map(y=>y.start))),xOfYear(Math.max(...years.map(y=>y.end)))];
        out.push({id:`time:${era.legacyId??era.id}`,variant:'era',strip:index%2?YEAR_ROWS.era1:YEAR_ROWS.era0,span,title:pick(era.title,locale),subtitle:'',center:[(span[0]+span[1])/2,0,0]});
      });
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
export function zarathustraTimeline({materials=[],references={},clusters=[]}={}){
  const placeOf=new Map(),outside=new Set();
  for(const material of materials){
    const grounds=(material.inquiry?.grounds??[]).map(ground=>references?.[ground?.ref]).filter(Boolean);
    if(!grounds.length)continue;
    const places=grounds.map(bookPlace).filter(Boolean);
    if(places.length)placeOf.set(material.id,places.reduce((a,b)=>b.chapter<a.chapter?b:a));else outside.add(material.id);
  }
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
    kind:'book',positions,lanes,domain:{min:0,max:maxChapter},rows:BOOK_ROWS,strip:BOOK_ROWS.era0+4,bounds:[xOfChapter(0),outside.size?outsideX:xOfChapter(maxChapter)],
    xOf,placeOf:id=>placeOf.get(id)??null,isOutside:id=>outside.has(id),
    labels(locale,{laneWidth=null}={}){
      const en=lang(locale)==='en',out=[];
      for(const chapter of marked)out.push({id:`time:ch${chapter}`,variant:'axis',strip:BOOK_ROWS.axis,title:chapter===0?(en?'Prologue':'Пролог'):toRoman(chapter),subtitle:'',center:[xOfChapter(chapter),0,0]});
      // A part spans the chapters the map cites from it; a rule stands between parts and before the texts outside the book,
      // whose column is named on the line of chapters, since it has none.
      ordered.forEach(([part,list],index)=>{
        const span=[xOfChapter(Math.min(...list)),xOfChapter(Math.max(...list))];
        out.push({id:`time:part${part}`,variant:'era',strip:BOOK_ROWS.era0,span,title:en?`Part ${toRoman(part)}`:`Часть ${toRoman(part)}`,subtitle:'',center:[(span[0]+span[1])/2,0,0]});
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

function toRoman(value){
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
