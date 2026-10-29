// A bonsai's places: where a dated situation happened, with the source that says so.
//
// A place carries its label, its gazetteer identities (Pleiades for the
// ancient world, Wikidata otherwise) and the representative point the
// gazetteer publishes, for display only. A situation's place says on what
// basis it stands: the source states where the work was written
// (`composition`) or where the event took place (`event`), or it states only
// where the author lived and worked in the dated span (`activity`), from which
// the place is inferred. Where the sources leave the place open, the entry says
// so (`unknown`) with what they do say, and names no place.

import {TREE_ID} from './time.mjs';

export const PLACES_SCHEMA='tos.bonsai.places.v1';
export const PLACE_BASES=Object.freeze(['composition','event','activity','unknown']);
const SCHEMES=Object.freeze(['pleiades','wikidata']);
const FOOTINGS=Object.freeze(['high','medium','low']);

const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const bilingual=value=>object(value)&&typeof value.ru==='string'&&value.ru.trim()&&typeof value.en==='string'&&value.en.trim();
const https=value=>typeof value==='string'&&/^https:\/\/\S+$/.test(value);

/** Where a places module departs from its own contract; empty when it holds. Situations must exist in `chronology` when it is given. */
export function placesIssues(module,chronology=null){
  if(!object(module))return ['not an object'];
  const issues=[];
  if(module.schema!==PLACES_SCHEMA)issues.push('schema');
  const sources=object(module.sources)?module.sources:{},places=object(module.places)?module.places:{},situations=object(module.situations)?module.situations:{};
  for(const [id,source] of Object.entries(sources))if(!object(source)||typeof source.title!=='string'||typeof source.short!=='string'||!https(source.url))issues.push(`source ${id}`);
  for(const [id,place] of Object.entries(places)){
    const at=`place ${id}`;
    if(!bilingual(place?.label))issues.push(`${at}: label`);
    const gazetteer=Array.isArray(place?.gazetteer)?place.gazetteer:[];
    if(!gazetteer.length)issues.push(`${at}: gazetteer`);
    for(const entry of gazetteer)if(!SCHEMES.includes(entry?.scheme)||typeof entry?.id!=='string'||!entry.id||!https(entry?.url))issues.push(`${at}: gazetteer entry`);
    const [lon,lat]=Array.isArray(place?.point)?place.point:[];
    if(!(Number.isFinite(lon)&&Number.isFinite(lat)&&Math.abs(lon)<=180&&Math.abs(lat)<=90))issues.push(`${at}: point`);
    if(!https(place?.point_source))issues.push(`${at}: point_source`);
  }
  const used=new Set();
  for(const [id,entry] of Object.entries(situations)){
    const at=`situation ${id}`;
    if(!TREE_ID.test(id))issues.push(`${at}: key is not a Tree identity`);
    if(chronology&&!chronology.situations?.[id])issues.push(`${at}: not a situation of the chronology`);
    if(!PLACE_BASES.includes(entry?.basis))issues.push(`${at}: basis`);
    if(entry?.basis==='unknown'){if(entry.place!==undefined)issues.push(`${at}: an unknown place names no place`);}
    else if(!places[entry?.place])issues.push(`${at}: place ${entry?.place} is not a place of this module`);
    else used.add(entry.place);
    if(entry?.basis==='event'&&chronology&&chronology.situations?.[id]?.type!=='historical-event')issues.push(`${at}: only an event takes place on the basis "event"`);
    if(!bilingual(entry?.statement))issues.push(`${at}: statement`);
    if(!FOOTINGS.includes(entry?.footing))issues.push(`${at}: footing`);
    if(!Array.isArray(entry?.sources)||!entry.sources.length)issues.push(`${at}: sources`);
    else for(const key of entry.sources)if(!sources[key])issues.push(`${at}: unknown source ${key}`);
  }
  for(const id of Object.keys(places))if(!used.has(id))issues.push(`place ${id}: no situation stands there`);
  return issues;
}

/** Read a places module; every lookup is safe on missing material. */
export function readPlaces(module){
  const places=object(module?.places)?module.places:{},situations=object(module?.situations)?module.situations:{},sources=object(module?.sources)?module.sources:{};
  return {
    module,
    place:id=>places[id]??null,
    /** What the module says about where a situation happened, placed or not. */
    entry:id=>situations[id]??null,
    /** The place a situation stands at, with its point; null where none is sourced. */
    at:id=>{const entry=situations[id];return entry?.place&&places[entry.place]?{...entry,point:places[entry.place].point}:null;},
    sources:id=>(situations[id]?.sources??[]).map(key=>object(sources[key])?{id:key,...sources[key]}:null).filter(Boolean),
  };
}
