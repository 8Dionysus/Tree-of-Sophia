// The witnesses of a work as a bonsai keeps them: per chapter, the exact units
// of each witness (the pivot — the original — and its translations) and the
// rows that align them, with the sentence pairs inside each row.
//
// A unit is [id, kind, raw, section] ("p" prose, "v" verse; the pivot's units
// add the ids of the public corpus units they map onto). `raw` is the unit's
// text exactly as its source gives it; offsets count its characters. What a
// reader sees is derived from it: a printed line end joins the words around it,
// a word broken at the end of a line ("¬") is joined again, verse keeps its
// lines. Nothing here rewrites a source.
//
// A citation names a place in the pivot: the chapter, the first and last unit,
// and optionally where the place starts in the first unit and ends in the last.
// The same place in a translation is found through the rows and their sentence
// pairs, never chosen by hand.

export const WITNESSES_SCHEMA='tos.bonsai.witnesses.v1';
export const CHAPTER_SCHEMA='tos.bonsai.witnesses.chapter.v1';

/** The units of a chapter file as objects, with an index by id per language. */
export function readChapter(file){
  const units=Object.fromEntries(Object.entries(file?.units??{}).map(([language,list])=>[language,list.map(([id,kind,raw,section,corpus])=>({id,kind,raw,section:section??null,...(corpus?{corpus}:{})}))]));
  const index=Object.fromEntries(Object.entries(units).map(([language,list])=>[language,new Map(list.map((unit,i)=>[unit.id,i]))]));
  return {key:file?.chapter,titles:file?.titles??{},units,rows:file?.rows??[],omitted:file?.omitted??{},index};
}

const HYPHEN=/¬[ \t]*\r?\n[ \t]*/g,HYPHEN_AT=/¬[ \t]*\r?\n[ \t]*/y;
/** The text the aligner compares: words joined again, whitespace collapsed, verse lines joined with " / ". */
export function readingText(raw,kind){
  const joined=String(raw??'').replace(HYPHEN,'');
  if(kind==='v')return joined.split(/\r?\n/).map(line=>line.replace(/\s+/g,' ').trim()).filter(Boolean).join(' / ');
  return joined.replace(/\s+/g,' ').trim();
}

/**
 * What a reader sees of a unit, with the way back from a raw offset to the
 * shown text: a broken word is joined, a line end becomes a space (verse keeps
 * it as a line), runs of whitespace become one, the ends are trimmed.
 */
export function displayText(raw,kind){
  const source=String(raw??''),shown=[],at=new Array(source.length+1);
  let i=0;
  while(i<source.length){
    at[i]=shown.length;
    HYPHEN_AT.lastIndex=i;
    const hyphen=source[i]==='¬'?HYPHEN_AT.exec(source):null;
    if(hyphen){for(let k=i;k<i+hyphen[0].length;k++)at[k]=shown.length;i+=hyphen[0].length;continue;}
    if(/\s/.test(source[i])){
      let j=i;while(j<source.length&&/\s/.test(source[j]))j++;
      const breaks=kind==='v'&&/\n/.test(source.slice(i,j));
      for(let k=i;k<j;k++)at[k]=shown.length;
      if(shown.length&&j<source.length)shown.push(breaks?'\n':' ');
      i=j;continue;
    }
    shown.push(source[i]);i++;
  }
  at[source.length]=shown.length;
  const text=shown.join('');
  return {text,at:offset=>at[Math.max(0,Math.min(source.length,offset))]??text.length};
}

/** Problems of one chapter file: units, rows covering every unit once, sentence spans inside their units. */
export function chapterIssues(file,{pivot='de',languages=['de','en','ru']}={}){
  const problems=[],chapter=readChapter(file),key=chapter.key??'?';
  if(file?.schema!==CHAPTER_SCHEMA)problems.push(`${key}: schema is not ${CHAPTER_SCHEMA}`);
  if(!chapter.units[pivot]?.length)problems.push(`${key}: no ${pivot} units`);
  const seen=new Set();
  for(const [language,list] of Object.entries(chapter.units)){
    if(!languages.includes(language))problems.push(`${key}: unknown language ${language}`);
    for(const unit of list){
      if(typeof unit.id!=='string'||!unit.id)problems.push(`${key}: ${language} unit without id`);
      if(seen.has(unit.id))problems.push(`${key}: duplicate unit ${unit.id}`);seen.add(unit.id);
      if(!['p','v'].includes(unit.kind))problems.push(`${key}: ${unit.id} has kind ${unit.kind}`);
      if(typeof unit.raw!=='string'||!unit.raw.trim())problems.push(`${key}: ${unit.id} has no text`);
      if(/[\uD800-\uDFFF]/.test(unit.raw??''))problems.push(`${key}: ${unit.id} holds a character outside the Basic Multilingual Plane, offsets would not count characters`);
    }
  }
  const omittedLanguages=Object.keys(chapter.omitted);
  for(const language of omittedLanguages)if(chapter.units[language]?.length)problems.push(`${key}: ${language} marked omitted but has units`);
  const covered=Object.fromEntries(Object.keys(chapter.units).map(language=>[language,new Array(chapter.units[language].length).fill(0)]));
  chapter.rows.forEach((row,n)=>{
    for(const [language,indexes] of Object.entries(row)){
      if(language==='sentences')continue;
      const list=chapter.units[language];
      if(!list){problems.push(`${key} row ${n}: no ${language} units`);continue;}
      indexes.forEach((i,k)=>{if(!Number.isInteger(i)||i<0||i>=list.length)problems.push(`${key} row ${n}: ${language} index ${i}`);else covered[language][i]++;if(k&&indexes[k-1]>=i)problems.push(`${key} row ${n}: ${language} indexes out of order`);});
    }
    for(const [language,pairs] of Object.entries(row.sentences??{})){
      for(const [source,target] of pairs){
        for(const [side,spans] of [[pivot,source],[language,target]]){
          if(!spans?.length)problems.push(`${key} row ${n}: empty sentence side`);
          for(const [i,start,end] of spans??[]){
            if(!(row[side]??[]).includes(i))problems.push(`${key} row ${n}: ${side} sentence in unit ${i} outside the row`);
            const raw=chapter.units[side]?.[i]?.raw??'';
            if(!(Number.isInteger(start)&&Number.isInteger(end)&&0<=start&&start<end&&end<=raw.length))problems.push(`${key} row ${n}: ${side} sentence span ${start}-${end} outside unit ${i}`);
          }
        }
      }
    }
  });
  for(const [language,counts] of Object.entries(covered))counts.forEach((count,i)=>{if(count!==1)problems.push(`${key}: ${language} unit ${i} is in ${count} rows`);});
  return problems;
}

/** Where a citation's pivot place lies in a chapter: unit indexes and raw offsets. */
function pivotPlace(cite,chapter,pivot){
  const [first,last]=cite?.u??[];
  const a=chapter.index[pivot]?.get(first),b=chapter.index[pivot]?.get(last);
  if(a===undefined||b===undefined||b<a)return null;
  const units=chapter.units[pivot];
  const start=Number.isInteger(cite.s)?cite.s:0,end=Number.isInteger(cite.e)?cite.e:units[b].raw.length;
  if(start<0||start>units[a].raw.length||end<0||end>units[b].raw.length||(a===b&&end<=start))return null;
  return {a,b,start,end};
}

/** Problems of a citation against the chapter it names. */
export function citeIssues(cite,chapter,{pivot='de'}={}){
  if(!cite||typeof cite!=='object')return ['not a citation'];
  if(chapter.key&&cite.c&&cite.c!==chapter.key)return [`names chapter ${cite.c}, not ${chapter.key}`];
  return pivotPlace(cite,chapter,pivot)?[]:[`no place ${JSON.stringify(cite.u)} ${cite.s??''}-${cite.e??''} in ${chapter.key}`];
}

const overlaps=(i,start,end,place)=>(i>place.a||(i===place.a&&end>place.start))&&(i<place.b||(i===place.b&&start<place.end));

/**
 * The cited place in one witness: its units with the shown text and the part
 * of each that the place covers. In the pivot this is the citation itself; in a
 * translation it is what the sentence pairs of the rows give, or the whole
 * counterpart units where a row has no sentence pairs. `omitted` is true when
 * the translation leaves the place out.
 */
export function fragmentOf(cite,chapter,language,{pivot='de'}={}){
  const place=pivotPlace(cite,chapter,pivot);
  if(!place)return null;
  const shown=(list,i,from,to)=>{const unit=list[i],view=displayText(unit.raw,unit.kind);return {index:i,id:unit.id,kind:unit.kind,text:view.text,from:view.at(from),to:view.at(to)};};
  if(language===pivot){
    const list=chapter.units[pivot],units=[];
    for(let i=place.a;i<=place.b;i++)units.push(shown(list,i,i===place.a?place.start:0,i===place.b?place.end:list[i].raw.length));
    return {language,units,rows:rowsOf(chapter,place,pivot),omitted:false};
  }
  const list=chapter.units[language];
  const rows=rowsOf(chapter,place,pivot);
  if(!list?.length||chapter.omitted?.[language])return {language,units:[],rows,omitted:true};
  const spans=new Map();let partial=false;
  for(const n of rows){
    const row=chapter.rows[n],targets=row[language]??[];
    // A row the translation leaves out has nothing to show; the rest of the place still shows.
    if(!targets.length){partial=true;continue;}
    let found=false;const paired=new Map();
    for(const [source,target] of row.sentences?.[language]??[]){
      for(const [i,s,e] of source){if(!paired.has(i))paired.set(i,[]);paired.get(i).push([s,e]);}
      if(!source.some(([i,s,e])=>overlaps(i,s,e,place)))continue;
      found=true;
      for(const [i,s,e] of target){const old=spans.get(i);spans.set(i,old?[Math.min(old[0],s),Math.max(old[1],e)]:[s,e]);}
    }
    // A sentence of the place that found no partner (the translation joined or crossed paragraphs
    // there) would silently drop out: then the row's whole counterpart stands for the place.
    const unpaired=(row[pivot]??[]).some(i=>{
      if(i<place.a||i>place.b)return false;
      const raw=chapter.units[pivot][i].raw,from=i===place.a?place.start:0,to=i===place.b?place.end:raw.length,cover=paired.get(i)??[];
      let missing=0;for(let k=from;k<to;k++)if(!/\s/.test(raw[k])&&!cover.some(([s,e])=>k>=s&&k<e))missing++;
      return missing>3;
    });
    if(!found||unpaired)for(const i of targets)spans.set(i,[0,list[i].raw.length]);
  }
  const indexes=[...spans.keys()].sort((x,y)=>x-y);
  if(!indexes.length)return {language,units:[],rows,omitted:true};
  // The counterpart reads as one run: from the first covered sentence to the last.
  const units=[];
  for(let i=indexes[0];i<=indexes.at(-1);i++){
    const span=spans.get(i)??[0,list[i].raw.length];
    units.push(shown(list,i,i===indexes[0]?span[0]:0,i===indexes.at(-1)?span[1]:list[i].raw.length));
  }
  return {language,units,rows,omitted:false,...(partial?{partial:true}:{})};
}

/** The rows that hold the pivot units of a place. */
function rowsOf(chapter,place,pivot){
  const rows=[];
  chapter.rows.forEach((row,n)=>{if((row[pivot]??[]).some(i=>i>=place.a&&i<=place.b))rows.push(n);});
  return rows;
}

/** Problems of a witnesses manifest and its chapters: every chapter listed, read and sound, counts agreeing. */
export function witnessesIssues(manifest,files){
  const problems=[];
  if(manifest?.schema!==WITNESSES_SCHEMA)problems.push(`manifest schema is not ${WITNESSES_SCHEMA}`);
  const pivot=manifest?.pivot,languages=Object.keys(manifest?.witnesses??{});
  if(!languages.includes(pivot))problems.push(`pivot ${pivot} is not a witness`);
  for(const [language,witness] of Object.entries(manifest?.witnesses??{})){
    for(const field of ['label','edition','licence','attribution','rights'])if(!witness?.[field])problems.push(`${language} witness has no ${field}`);
    if(!['public-domain','public-domain+licence','short-quotation'].includes(witness?.rights?.basis))problems.push(`${language} witness has no rights basis`);
  }
  for(const entry of manifest?.chapters??[]){
    const file=files[entry.key];
    if(!file){problems.push(`chapter ${entry.key} missing`);continue;}
    problems.push(...chapterIssues(file,{pivot,languages}));
    for(const [language,count] of Object.entries(entry.units??{}))if((file.units?.[language]?.length??0)!==count)problems.push(`chapter ${entry.key}: ${language} has ${file.units?.[language]?.length??0} units, manifest says ${count}`);
  }
  for(const key of Object.keys(files))if(!(manifest?.chapters??[]).some(entry=>entry.key===key))problems.push(`chapter ${key} not in the manifest`);
  return problems;
}
