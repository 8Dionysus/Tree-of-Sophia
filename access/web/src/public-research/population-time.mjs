// The lens "Time" for a population: each member stands at the years of its life, or of its activity where no life is
// known, as the bonsai's chronology dates them; a member known by one bound only stands at that year; a member whose
// years are unknown stands off the scale. Rows are the regions of the members' founding nodes. Centuries with more
// members get more room, each still linear inside, every one named; the width of a century says nothing of anyone's
// weight. The layout is display only.
import {readChronology} from '../../../../bonsai/lib/chronology.mjs';
import {centuryStart,formatYears} from '../../../../bonsai/lib/time.mjs';
import {copyLocale,localeText,localeValue} from './locale.mjs';
import {homeOf,memberMaterial,REGION_NAMES,REGION_ORDER} from './population-library.mjs';
import {centuryLabel,stackColumns,YEAR_ROWS} from './space-timeline.mjs';

const LANE=40,STACK=3.2,WIDTH=1100;
const yearOfLexeme=value=>{const match=/^(-?\d{4,})/.exec(String(value??''));return match?Number.parseInt(match[1],10):null;};

/** A member's span on the scale: its life or activity, else the one bound a phase gives, else null. */
export function memberSpan(member,{chronology,reader}){
  const life=chronology.lives?.[member.id];
  const span=life?reader.span(life):null;
  if(span)return {...span,situation:life};
  for(const phase of member.phases??[]){
    const dating=chronology.situations?.[phase]?.dating,interval=dating?.interval;
    const year=dating?.kind==='date-assertion'?yearOfLexeme(dating.value):interval?yearOfLexeme(interval.start??interval.end):null;
    // One year spans that year, [year, year+1), as a dated life's years do (lexemeBounds).
    if(Number.isFinite(year))return {from:year,to:year+1,situation:phase,certainty:dating.certainty,open:dating.kind!=='date-assertion'};
  }
  return null;
}

export function populationTimeline({population,chronology}){
  const reader=readChronology(chronology),timeOf=new Map(),undated=new Set(),rowOf=new Map();
  for(const [id,member] of Object.entries(population.members)){
    const materialId=memberMaterial(id),span=memberSpan({...member,id},{chronology,reader});
    rowOf.set(materialId,population.batches[homeOf(member)]?.node?.region??'transnational');
    if(span)timeOf.set(materialId,{span,situation:span.situation,memberId:id});else undated.add(materialId);
  }
  if(!timeOf.size)return null;
  const spans=[...timeOf.values()].map(time=>time.span);
  const min=centuryStart(Math.floor(Math.min(...spans.map(span=>span.from)))),max=centuryStart(Math.ceil(Math.max(...spans.map(span=>span.to)))-1)+100;
  // A member stands at the middle of its span.
  const yearOf=id=>{const span=timeOf.get(id)?.span;return span?(span.from+span.to)/2:null;};
  const counts=new Map();for(const id of timeOf.keys()){const start=centuryStart(Math.floor(yearOf(id)));counts.set(start,(counts.get(start)??0)+1);}
  const centuries=Array.from({length:(max-min)/100},(_,i)=>min+i*100);
  const weights=centuries.map(start=>1+2*Math.sqrt(counts.get(start)??0)),total=weights.reduce((a,b)=>a+b,0);
  const offsets=[0];for(const weight of weights)offsets.push(offsets.at(-1)+weight/total*WIDTH);
  const xOfYear=year=>{const index=Math.max(0,Math.min(centuries.length-1,Math.floor((year-min)/100)));return -WIDTH/2+offsets[index]+(year-centuries[index])/100*(offsets[index+1]-offsets[index]);};
  const outsideX=WIDTH/2+120,regions=REGION_ORDER.filter(key=>[...rowOf.values()].includes(key));
  const positions=new Map(),lanes=[];
  for(const [index,region] of regions.entries()){
    const ids=[...rowOf].filter(([,key])=>key===region).map(([id])=>id),y=index*LANE;
    const xs=new Map(ids.filter(id=>timeOf.has(id)).map(id=>[id,xOfYear(yearOf(id))]));
    ids.filter(id=>undated.has(id)).forEach((id,i)=>xs.set(id,outsideX+(i%24)*5));
    // Members that meet on the row stand in short columns above and below its line.
    const dys=stackColumns(xs,2.4,STACK);
    for(const [id,x] of xs)positions.set(id,[x,y+dys.get(id),0]);
    const inside=[...xs].filter(([id])=>timeOf.has(id)).map(([,x])=>x);
    lanes.push({id:region,title:REGION_NAMES[region],y,from:inside.length?Math.min(...inside):outsideX,to:inside.length?Math.max(...inside):outsideX});
  }
  const shift=-(lanes.length-1)*LANE/2;for(const position of positions.values())position[1]+=shift;for(const lane of lanes)lane.y+=shift;
  return {
    kind:'years',positions,lanes,domain:{min,max},rows:YEAR_ROWS,strip:YEAR_ROWS.era0+4,bounds:[xOfYear(min),undated.size?outsideX:xOfYear(max)],
    xOf:id=>timeOf.has(id)?xOfYear(yearOf(id)):undated.has(id)?outsideX:null,xOfYear,
    timeOf:id=>timeOf.get(id)??null,spanOf:id=>timeOf.get(id)?.span??null,isOutside:id=>undated.has(id),reader,
    scaleEnds:locale=>[formatYears({first:min,last:min,certainty:'exact'},locale),formatYears({first:max-1,last:max-1,certainty:'exact'},locale)],
    lifeSpan:()=>null,
    labels(locale,{laneWidth=null}={}){
      const en=copyLocale(locale)==='en',out=[];
      for(let year=min;year<=max;year+=100)out.push({id:`time:rule:${year}`,variant:'rule',title:'',subtitle:'',center:[xOfYear(year),0,0],...(year===1?{tone:'strong'}:{})});
      for(let start=min;start<max;start+=100)out.push({id:`time:c${start}`,variant:'axis',strip:YEAR_ROWS.axis,title:centuryLabel(start,locale),subtitle:'',center:[xOfYear(start+50),0,0],noOverlap:true});
      if(min<1&&max>1)out.push({id:'time:bce',variant:'side',align:'end',strip:YEAR_ROWS.side,title:en?localeText('BCE',locale):'до н. э.',subtitle:'',center:[xOfYear(1),0,0]},{id:'time:ce',variant:'side',align:'start',strip:YEAR_ROWS.side,title:en?localeText('CE',locale):'н. э.',subtitle:'',center:[xOfYear(1),0,0]});
      if(undated.size){
        out.push({id:'time:rule:undated',variant:'rule',tone:'strong',title:'',subtitle:'',center:[(xOfYear(max)+outsideX)/2,0,0]});
        out.push({id:'time:undated',variant:'side',strip:YEAR_ROWS.axis,title:en?localeText('date unknown',locale):'дата неизвестна',subtitle:'',center:[outsideX,0,0]});
      }
      for(const lane of lanes){
        out.push({id:`time:thread:${lane.id}`,variant:'thread',title:'',subtitle:'',span:[lane.from,lane.to],center:[(lane.from+lane.to)/2,lane.y,0]});
        out.push({id:`time:lane:${lane.id}`,variant:'lane',title:localeValue(lane.title,locale),subtitle:'',center:[lane.from,lane.y,0],...(laneWidth?{maxWidth:laneWidth}:{})});
      }
      return out;
    },
  };
}
