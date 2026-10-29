// Historical time in the Tree's own grammar.
//
// A bonsai keeps every dating as a value of the Tree's `historicalDate`
// (ToS/contracts/historical-claim.schema.json#/$defs/historicalDate): a date, an
// interval, an order relative to another dated situation, or an explicit
// unknown, each with its certainty and the source's own wording. Years are
// counted astronomically (1 BCE = 0, 2 BCE = −1), as the Tree requires of
// comparable values; BCE/CE wording is made only for the reader.

export const DATE_KINDS=Object.freeze(['date-assertion','interval-assertion','relative-order','unknown-date']);
export const CERTAINTIES=Object.freeze(['exact','approximate','uncertain','unknown']);
/** The weaker of two certainties, in the order of CERTAINTIES; an unknown value leaves the other. */
export function weakerCertainty(a,b){
  const i=CERTAINTIES.indexOf(a),j=CERTAINTIES.indexOf(b);
  return i<0?(j<0?null:b):j<0?a:CERTAINTIES[Math.max(i,j)];
}
export const RELATIONS=Object.freeze(['before','after','during','overlaps']);
/** Identity syntax of a Tree reference; the Tree's own registry decides its type. */
export const TREE_ID=/^tos\.[a-z][a-z0-9-]*\.[a-z0-9]+(?:[.-][a-z0-9]+)*$/;
const LEXEME=/^(-?)(\d{4})(?:-(\d{2})(?:-(\d{2}))?)?$/;
const FIELDS=new Set(['kind','role','calendar','year_numbering','certainty','source_wording','value','interval','relative','extensions']);

const mod=(a,n)=>((a%n)+n)%n;
const leap=year=>mod(year,4)===0&&(mod(year,100)!==0||mod(year,400)===0);
const daysIn=(year,month)=>[31,leap(year)?29:28,31,30,31,30,31,31,30,31,30,31][month-1];
const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);

/** The parts of a Tree date lexeme ("-0386", "1883-07", "1883-07-01"), or null. */
export function lexemeParts(text){
  const match=LEXEME.exec(typeof text==='string'?text:'');if(!match)return null;
  const year=Number(match[2])*(match[1]?-1:1)||0,month=match[3]?Number(match[3]):null,day=match[4]?Number(match[4]):null;
  if(month!==null&&(month<1||month>12))return null;
  if(day!==null&&(day<1||day>daysIn(year,month)))return null;
  return {year,month,day,precision:day!==null?'day':month!==null?'month':'year'};
}

/** A whole astronomical year as a Tree lexeme: −386 → "-0386", 44 → "0044". */
export function yearLexeme(year){
  if(!Number.isInteger(year)||Math.abs(year)>9999)throw new TypeError('A whole year within four digits is required.');
  return `${year<0?'-':''}${String(Math.abs(year)).padStart(4,'0')}`;
}

/** A historian's year, which has no year zero, in astronomical numbering. */
export const astronomicalYear=(year,era)=>era==='BCE'?1-year:year;
/** An astronomical year as a historian writes it. */
export const historicalYear=year=>year<=0?{year:1-year,era:'BCE'}:{year,era:'CE'};

// The first and last day a lexeme's parts name, as [year, month, day], and their order.
const firstDay=parts=>[parts.year,parts.month??1,parts.day??1];
const lastDay=parts=>{const month=parts.month??12;return [parts.year,month,parts.day??daysIn(parts.year,month)];};
const compareDays=(a,b)=>a[0]-b[0]||a[1]-b[1]||a[2]-b[2];

/** The first and last moments a lexeme names, as fractional astronomical years: [from, to). */
export function lexemeBounds(text){
  const parts=lexemeParts(text);if(!parts)return null;
  if(parts.precision==='year')return [parts.year,parts.year+1];
  const month=parts.year+(parts.month-1)/12;
  if(parts.precision==='month')return [month,month+1/12];
  const day=1/12/daysIn(parts.year,parts.month);
  return [month+(parts.day-1)*day,month+parts.day*day];
}

/**
 * Where a value departs from the Tree's historicalDate grammar; empty when it
 * follows it. With astronomical numbering its lexemes must also be readable
 * and an interval's bounds ordered.
 */
export function dateIssues(value){
  if(!object(value))return ['not an object'];
  const issues=[],has=key=>Object.hasOwn(value,key);
  for(const key of Object.keys(value))if(!FIELDS.has(key))issues.push(`unknown field: ${key}`);
  if(!DATE_KINDS.includes(value.kind))issues.push('kind');
  if(value.role!=='historical-time')issues.push('role');
  for(const key of ['calendar','year_numbering'])if(!has(key)||(value[key]!==null&&(typeof value[key]!=='string'||!value[key])))issues.push(key);
  if(!CERTAINTIES.includes(value.certainty))issues.push('certainty');
  const wording=value.source_wording;
  if(!object(wording)||typeof wording.text!=='string'||!/\S/.test(wording.text)||!Object.hasOwn(wording,'language')||(wording.language!==null&&(typeof wording.language!=='string'||!wording.language))||Object.keys(wording).some(key=>key!=='text'&&key!=='language'))issues.push('source_wording');
  if(has('extensions')&&!object(value.extensions))issues.push('extensions');
  const astronomical=value.year_numbering==='astronomical';
  const lexeme=(text,label)=>{if(typeof text!=='string'||!/\S/.test(text))issues.push(label);else if(astronomical&&!lexemeParts(text))issues.push(`${label}: not a Tree lexeme`);};
  if(value.kind==='date-assertion'){if(has('interval')||has('relative'))issues.push('a date carries no interval or relative order');lexeme(value.value,'value');}
  else if(value.kind==='interval-assertion'){
    if(has('value')||has('relative'))issues.push('an interval carries no value or relative order');
    const interval=value.interval;
    if(!object(interval)||(!Object.hasOwn(interval,'start')&&!Object.hasOwn(interval,'end'))||Object.keys(interval).some(key=>key!=='start'&&key!=='end'))issues.push('interval');
    else{
      if(Object.hasOwn(interval,'start'))lexeme(interval.start,'interval.start');
      if(Object.hasOwn(interval,'end'))lexeme(interval.end,'interval.end');
      // An interval is out of order only where its start begins after its end is over: "1883-07-15" to "1883-07" holds.
      // The days are compared whole, never as fractions of a year, which round differently on either side.
      const a=astronomical?lexemeParts(interval.start):null,b=astronomical?lexemeParts(interval.end):null;
      if(a&&b&&compareDays(firstDay(a),lastDay(b))>0)issues.push('interval: start after end');
    }
  }else if(value.kind==='relative-order'){
    if(has('value')||has('interval'))issues.push('a relative order carries no value or interval');
    const relative=value.relative;
    if(!object(relative)||!RELATIONS.includes(relative.relation)||typeof relative.anchor_ref!=='string'||!TREE_ID.test(relative.anchor_ref)||Object.keys(relative).some(key=>key!=='relation'&&key!=='anchor_ref'))issues.push('relative');
  }else if(value.kind==='unknown-date'){
    if(has('value')||has('interval')||has('relative'))issues.push('an unknown date carries no value');
    if(value.certainty!=='unknown')issues.push('an unknown date has unknown certainty');
  }
  return issues;
}

/**
 * The years a dating spans, for display: [from, to) in astronomical years, the
 * closed years first..last it names, and how it was reached. A relative
 * "during" takes its anchor's span through `anchorSpan(ref)`, and is no firmer
 * than that span: its `certainty` is the weaker of its own and the anchor's,
 * its own stays as `ownCertainty`. "Before", "after" and "overlaps" bound only
 * one side or neither, so they give no span, nor does an unknown date or an
 * open interval.
 */
export function dateSpan(value,anchorSpan=()=>null){
  if(!object(value)||value.year_numbering!=='astronomical'&&value.kind!=='relative-order')return null;
  const base={kind:value.kind,certainty:value.certainty};
  if(value.kind==='date-assertion'){
    const bounds=lexemeBounds(value.value),parts=lexemeParts(value.value);
    return bounds?{...base,from:bounds[0],to:bounds[1],first:parts.year,last:parts.year,via:[]}:null;
  }
  if(value.kind==='interval-assertion'){
    const a=lexemeBounds(value.interval?.start),b=lexemeBounds(value.interval?.end);
    return a&&b?{...base,from:a[0],to:b[1],first:lexemeParts(value.interval.start).year,last:lexemeParts(value.interval.end).year,via:[]}:null;
  }
  if(value.kind==='relative-order'&&value.relative?.relation==='during'){
    const anchor=anchorSpan(value.relative.anchor_ref);
    return anchor?{...base,certainty:weakerCertainty(value.certainty,anchor.certainty),ownCertainty:value.certainty,from:anchor.from,to:anchor.to,first:anchor.first,last:anchor.last,relation:'during',anchor:value.relative.anchor_ref,anchorCertainty:anchor.certainty,via:[value.relative.anchor_ref,...(anchor.via??[])]}:null;
  }
  return null;
}

const lang=locale=>String(locale||'ru').startsWith('en')?'en':'ru';
// Words are joined to their numbers by no-break spaces, so "387 до н. э." never breaks inside.
const NB=' ';
const MONTHS={
  ru:{nominative:['январь','февраль','март','апрель','май','июнь','июль','август','сентябрь','октябрь','ноябрь','декабрь'],genitive:['января','февраля','марта','апреля','мая','июня','июля','августа','сентября','октября','ноября','декабря']},
  en:{nominative:['January','February','March','April','May','June','July','August','September','October','November','December']},
};
// A dating whose firmness is unknown is marked as doubtful, never shown as exact.
const firmness=(certainty,en)=>({about:certainty==='approximate'?(en?`c.${NB}`:`ок.${NB}`):'',doubt:certainty==='uncertain'||certainty==='unknown'?`${NB}(?)`:''});
// A year of the common era from 1000 on needs no era; earlier ones and every year before it do.
const eraOf=(value,en)=>value.era==='BCE'?(en?'BCE':`до${NB}н.${NB}э.`):value.year>=1000?'':(en?'CE':`н.${NB}э.`);
const withEra=(text,value,en)=>{const era=eraOf(value,en);return era?`${text}${NB}${era}`:text;};

/**
 * Closed years in the reader's language, as firm as their dating:
 * "ок. 399–387 до н. э.", "44 BCE", "c. 108 CE", "1 BCE – 65 CE", "253–270 н. э. (?)", "1883–1885".
 */
export function formatYears({first,last,certainty}={},locale){
  if(!Number.isInteger(first)||!Number.isInteger(last))return '';
  const en=lang(locale)==='en',{about,doubt}=firmness(certainty,en);
  const a=historicalYear(Math.min(first,last)),b=historicalYear(Math.max(first,last));
  if(a.era===b.era&&!!eraOf(a,en)===!!eraOf(b,en))return `${about}${withEra(a.year===b.year?String(a.year):`${a.year}–${b.year}`,b,en)}${doubt}`;
  return `${about}${withEra(String(a.year),a,en)} – ${withEra(String(b.year),b,en)}${doubt}`;
}

/** One moment of a lexeme in words: "1714", "январь 1884", "16 октября 1843", "c. 500 BCE"; `grammar` 'genitive' after "с"/"до". */
function moment(parts,en,{grammar='nominative',year=true}={}){
  const value=historicalYear(parts.year),months=en?MONTHS.en.nominative:MONTHS.ru[parts.day!==null?'genitive':grammar];
  const month=parts.month!==null?months[parts.month-1]:'';
  const head=parts.day!==null?(en?`${parts.day}${NB}${month}`:`${parts.day}${NB}${month}`):month;
  if(!year)return head;
  return withEra(head?`${head}${NB}${value.year}`:String(value.year),value,en);
}

/**
 * A dating in the reader's language, as firm as it is stated: a date ("январь 1884"),
 * a closed interval ("январь–февраль 1883", "1–15 июля 1883", "ноябрь 1884 – февраль 1885",
 * "ок. 387–380 до н. э."), an open one ("до 1962", "примерно с 1663", "from c. 1663").
 * A relative dating speaks through the span of its anchor, given as `span`, and is no
 * firmer than that span; an unknown date has no words here.
 */
export function formatDating(value,locale,{span=null}={}){
  if(!object(value))return '';
  const en=lang(locale)==='en';
  if(value.kind==='relative-order')return span?formatYears({...span,certainty:weakerCertainty(value.certainty,span.certainty)},locale):'';
  if(value.kind==='unknown-date'||value.year_numbering!=='astronomical')return '';
  const {about,doubt}=firmness(value.certainty,en);
  // "Около" takes the genitive in Russian: "ок. января 1884", "ок. ноября 1884 – февраля 1885".
  const grammar=about&&!en?'genitive':'nominative';
  if(value.kind==='date-assertion'){const parts=lexemeParts(value.value);return parts?`${about}${moment(parts,en,{grammar})}${doubt}`:'';}
  if(value.kind!=='interval-assertion')return '';
  const a=lexemeParts(value.interval?.start),b=lexemeParts(value.interval?.end);
  if(a&&b){
    if(a.precision==='year'&&b.precision==='year')return formatYears({first:a.year,last:b.year,certainty:value.certainty},locale);
    // Bounds of one precision within one year say the year once, and within one month the month too;
    // bounds of unlike precision are each said in full.
    if(a.year===b.year&&a.precision===b.precision){
      if(a.month===b.month)return a.precision==='month'||a.day===b.day?`${about}${moment(a,en,{grammar})}${doubt}`:`${about}${a.day}–${moment(b,en,{grammar})}${doubt}`;
      return `${about}${moment(a,en,{year:false,grammar})}${a.precision==='day'?' – ':'–'}${moment(b,en,{grammar})}${doubt}`;
    }
    return `${about}${moment(a,en,{grammar})} – ${moment(b,en,{grammar})}${doubt}`;
  }
  // An open bound is as firm as the dating: "с февраля 1941", "примерно до 1962", "from c. 1663".
  const open=(ru,word,parts)=>`${en?`${word}${NB}${about}`:`${about?`примерно${NB}`:''}${ru}${NB}`}${moment(parts,en,{grammar:'genitive'})}${doubt}`;
  if(a)return open('с','from',a);
  if(b)return open('до','by',b);
  return '';
}

/** The century a year belongs to: −399 (400 BCE) → {n: 4, era: 'BCE'}, 44 → {n: 1, era: 'CE'}. */
export function centuryOf(year){
  const {year:value,era}=historicalYear(year);
  return {n:Math.ceil(value/100),era};
}
/** The first year of the century that holds `year`, astronomically: centuries begin at 100k + 1. */
export const centuryStart=year=>Math.floor((year-1)/100)*100+1;
