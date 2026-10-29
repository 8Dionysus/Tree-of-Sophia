import {resolveLocale,copyLocale,localeText,localeValue} from './locale.mjs';
// The lens "Space" measures where: a star stands at the place its bonsai says
// its material was written — or, where only that is known, where its author
// lived and worked in the dated span — at the gazetteer's representative
// point. Stars at one place gather round its point in a sunflower, so none
// hides another and the place stays where it is. A star without a sourced
// place stands apart, past the map's edge, never at a guessed point.
//
// The projection is equirectangular about the middle latitude φ₀: a degree of
// longitude is shortened by cos φ₀, so distances read nearly true across a
// regional map. Positions are display only and never enter the saved graph.

const lang=resolveLocale;
const pick=localeValue;
const GOLDEN=Math.PI*(3-Math.sqrt(5));
const EARTH_KM=6371.0088;

/** Great-circle distance in kilometres between two [longitude, latitude] points. */
export function greatCircleKm(a,b){
  if(!Array.isArray(a)||!Array.isArray(b))return null;
  const rad=Math.PI/180,[l1,f1]=a.map(v=>v*rad),[l2,f2]=b.map(v=>v*rad);
  const h=Math.sin((f2-f1)/2)**2+Math.cos(f1)*Math.cos(f2)*Math.sin((l2-l1)/2)**2;
  return 2*EARTH_KM*Math.asin(Math.min(1,Math.sqrt(h)));
}

/** Polylines cut at a vertical line: only the parts with x ≤ `limit` remain, each crossing ending exactly on the line. */
export function clipLinesRight(lines,limit){
  const out=[];
  for(const line of lines){
    let current=[];
    for(let i=0;i<line.length;i++){
      const p=line[i],inside=p[0]<=limit,prev=i?line[i-1]:null;
      if(prev&&(prev[0]<=limit)!==inside){const t=(limit-prev[0])/(p[0]-prev[0]),cross=[limit,prev[1]+t*(p[1]-prev[1])];current.push(cross);if(!inside){if(current.length>1)out.push(current);current=[];}}
      if(inside)current.push(p);
    }
    if(current.length>1)out.push(current);
  }
  return out;
}

/** The k-th of n points of a sunflower around the origin, `spacing` apart. */
export function sunflower(k,spacing){
  if(k===0)return [0,0];
  const r=spacing*Math.sqrt(k+.5)*.9,a=k*GOLDEN;
  return [r*Math.cos(a),r*Math.sin(a)];
}

const niceStep=span=>[1,2,5,10,15,20].find(step=>span/step<=7)??30;
const degrees=(value,positive,negative,locale)=>{
  const abs=Math.abs(value),word=value>=0?positive:negative;
  return `${new Intl.NumberFormat(resolveLocale(locale),{maximumFractionDigits:1}).format(abs)}° ${pick(word,locale)}`;
};
const EAST={ru:'в. д.',en:'E'},WEST={ru:'з. д.',en:'W'},NORTH={ru:'с. ш.',en:'N'},SOUTH={ru:'ю. ш.',en:'S'};

/**
 * A plane of places for `stars` in their order. `placeOf(id)` gives
 * `{place, basis}` or null; `places[place]` holds `{label, point:[lon, lat]}`.
 */
export function planeOfPlaces({stars=[],placeOf=()=>null,places={},width=1000,spacing=5.2}={}){
  const at=new Map(),outside=[];
  for(const id of stars){
    const own=placeOf(id),point=own?places[own.place]?.point:null;
    if(Array.isArray(point)&&point.length===2&&point.every(Number.isFinite))at.set(id,{...own,point});else outside.push(id);
  }
  if(!at.size)return null;
  const used=[...new Set([...at.values()].map(item=>item.place))],points=used.map(id=>places[id].point);
  const lons=points.map(p=>p[0]),lats=points.map(p=>p[1]);
  const margin=1.2,lonMin=Math.min(...lons)-margin,lonMax=Math.max(...lons)+margin,latMin=Math.min(...lats)-margin,latMax=Math.max(...lats)+margin;
  const phi0=(latMin+latMax)/2*Math.PI/180,k=width/Math.max(1e-6,(lonMax-lonMin)*Math.cos(phi0));
  const lon0=(lonMin+lonMax)/2,lat0=(latMin+latMax)/2;
  const toXY=([lon,lat])=>[(lon-lon0)*Math.cos(phi0)*k,-(lat-lat0)*k];
  const positions=new Map(),counts=new Map();
  for(const id of stars){
    const item=at.get(id);if(!item)continue;
    const n=counts.get(item.place)??0;counts.set(item.place,n+1);
    const [x,y]=toXY(item.point),[dx,dy]=sunflower(n,spacing);positions.set(id,[x+dx,y+dy,0]);
  }
  const [xMin,yMax]=toXY([lonMin,latMin]),[xMax,yMin]=toXY([lonMax,latMax]),outsideX=xMax+110;
  outside.forEach((id,index)=>positions.set(id,[outsideX+(index%4)*12,yMin+40+Math.floor(index/4)*14,0]));
  const lonStep=niceStep(lonMax-lonMin),latStep=niceStep(latMax-latMin);
  return {
    kind:'plane',positions,toXY,bounds:{x:[xMin,outside.length?outsideX+48:xMax],y:[yMin,yMax]},map:{x:[xMin,xMax],y:[yMin,yMax]},rows:{axis:22},strip:26,
    placeOf:id=>at.get(id)??null,isOutside:id=>outside.includes(id),counts,used,
    labels(locale){
      const en=copyLocale(locale)==='en',out=[];
      // The graticule: meridians named on the strip above the map, parallels at its left.
      for(let lon=Math.ceil(lonMin/lonStep)*lonStep;lon<=lonMax;lon+=lonStep){
        const [x]=toXY([lon,lat0]);
        out.push({id:`space:m${lon}`,variant:'rule',title:'',subtitle:'',center:[x,0,0]},{id:`space:mt${lon}`,variant:'axis',strip:22,title:degrees(lon,EAST,WEST,locale),subtitle:'',center:[x,0,0]});
      }
      for(let lat=Math.ceil(latMin/latStep)*latStep;lat<=latMax;lat+=latStep){
        const [,y]=toXY([lon0,lat]);
        out.push({id:`space:p${lat}`,variant:'hrule',title:'',subtitle:'',center:[0,y,0]},{id:`space:pt${lat}`,variant:'ytick',title:degrees(lat,NORTH,SOUTH,locale),subtitle:'',center:[xMin,y,0]});
      }
      // A place is named at its point, with how many stars stand there.
      // The place with more stars names itself first; a place known only as where the author lived speaks more quietly.
      for(const id of [...used].sort((a,b)=>counts.get(b)-counts.get(a))){
        const [x,y]=toXY(places[id].point),activity=[...at.values()].every(item=>item.place!==id||item.basis==='activity');
        out.push({id:`space:place:${id}`,variant:'place',title:`${pick(places[id].label,locale)} · ${counts.get(id)}`,subtitle:'',center:[x,y,0],...(activity?{tone:'quiet',hint:en?localeText('Where the author lived',locale):'Здесь жил автор'}:{})});
      }
      if(outside.length){
        out.push({id:'space:rule:outside',variant:'rule',tone:'strong',title:'',subtitle:'',center:[(xMax+outsideX)/2,0,0]});
        out.push({id:'space:outside',variant:'side',strip:22,title:en?localeText('place unknown',locale):'место не установлено',subtitle:'',center:[outsideX+18,0,0]});
      }
      return out;
    },
  };
}
