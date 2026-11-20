// The Population stand's reading of its bonsai: who the members are, when they lived and how a reader finds
// them. Pure functions over the bonsai's population.json and chronology.json, so the page and its tests read
// one model. Years are the Tree's astronomical years (1 BCE = 0); a member's words stay the registers' own.

const ERA={ru:'до н. э.',en:'BCE',es:'a. C.',de:'v. Chr.','zh-Hans':'公元前'};
const WORDS={
  ru:{about:'ок. ',born:'род. ',died:'ум. ',active:'деят. ',unknown:'годы неизвестны',doubt:' (?)'},
  en:{about:'c. ',born:'b. ',died:'d. ',active:'fl. ',unknown:'dates unknown',doubt:' (?)'},
  es:{about:'c. ',born:'n. ',died:'m. ',active:'fl. ',unknown:'fechas desconocidas',doubt:' (?)'},
  de:{about:'um ',born:'geb. ',died:'gest. ',active:'wirkte ',unknown:'Lebensdaten unbekannt',doubt:' (?)'},
  'zh-Hans':{about:'约',born:'生于',died:'卒于',active:'活跃于',unknown:'生卒年不详',doubt:'（存疑）'},
};
const words=locale=>WORDS[locale]??WORDS.en;

/** A year lexeme of the Tree (-?YYYY, -MM and -DD optional) as its astronomical year; null where there is none. */
export function yearOf(lexeme){
  const match=/^(-?)(\d{4})/.exec(String(lexeme??''));
  return match?(match[1]?-1:1)*Number(match[2]):null;
}

/** A year as a reader writes it: 1275, or 428 BCE for the astronomical -427. */
export function yearText(year,locale){
  if(year>0)return String(year);
  return locale==='zh-Hans'?`${ERA[locale]}${1-year}`:`${1-year} ${ERA[locale]??ERA.en}`;
}

/** The century a year falls in: 13 for 1275, -5 for 428 BCE (the astronomical -427). */
export function centuryOf(year){
  if(year===null||year===undefined)return null;
  return year>0?Math.floor((year-1)/100)+1:-(Math.floor(-year/100)+1);
}

const ROMAN=[[10,'X'],[9,'IX'],[5,'V'],[4,'IV'],[1,'I']];
const roman=value=>{let rest=value,out='';for(const [step,mark] of ROMAN)while(rest>=step){out+=mark;rest-=step;}return out;};
const ordinal=value=>{const tail=value%100>=11&&value%100<=13?'th':({1:'st',2:'nd',3:'rd'}[value%10]??'th');return `${value}${tail}`;};

/** A century as a reader names it: XIII в., 13th c., 5th c. BCE. */
export function centuryText(century,locale){
  const bce=century<0,value=Math.abs(century);
  switch(locale){
    case 'ru':return `${roman(value)} в.${bce?' до н. э.':''}`;
    case 'es':return `s. ${roman(value)}${bce?' a. C.':''}`;
    case 'de':return `${value}. Jh.${bce?' v. Chr.':''}`;
    case 'zh-Hans':return `${bce?'公元前':''}${value}世纪`;
    default:return `${ordinal(value)} c.${bce?' BCE':''}`;
  }
}

/** The phase that stands for a member's life: the chronology's own choice (`lives`), else its first dated phase. */
export function lifePhase(id,member,chronology){
  const situations=chronology.situations??{};
  const chosen=chronology.lives?.[id];
  if(chosen&&situations[chosen])return {id:chosen,...situations[chosen]};
  const dated=member.phases.find(phase=>situations[phase]&&situations[phase].dating.kind!=='unknown-date');
  const phase=dated??member.phases.find(phase=>situations[phase]);
  return phase?{id:phase,...situations[phase]}:null;
}

/** The years a phase dates, either bound null where the register leaves it open; null for an unknown date. */
export function spanOf(phase){
  if(!phase)return null;
  const {dating}=phase,activity=String(phase.id??'').endsWith('.activity');
  if(dating.kind==='interval-assertion'){
    const from=yearOf(dating.interval?.start),to=yearOf(dating.interval?.end);
    return from===null&&to===null?null:{from,to,certainty:dating.certainty,activity};
  }
  if(dating.kind==='date-assertion'){const year=yearOf(dating.value);return year===null?null:{from:year,to:year,certainty:dating.certainty,activity};}
  return null;
}

/** A span as a reader reads it: 1275–1344, c. 428–348 BCE, fl. 1300, b. 1873; dates unknown for none. */
export function spanText(span,locale){
  const w=words(locale);
  if(!span)return w.unknown;
  const {from,to}=span;
  const about=span.certainty==='approximate'?w.about:'',doubt=span.certainty==='uncertain'?w.doubt:'';
  // An open span keeps its word first: b. c. 1260.
  if(to===null)return `${w.born}${about}${yearText(from,locale)}${doubt}`;
  if(from===null)return `${w.died}${about}${yearText(to,locale)}${doubt}`;
  // Both bounds before the era: the era is named once, after the second.
  const years=from===to?yearText(from,locale)
    :from<=0&&to<=0?(locale==='zh-Hans'?`${ERA[locale]}${1-from}–${1-to}`:`${1-from}–${1-to} ${ERA[locale]??ERA.en}`)
    :`${yearText(from,locale)}–${yearText(to,locale)}`;
  return `${span.activity?w.active:''}${about}${years}${doubt}`;
}

/** Names folded for finding: no accents, one case, ё as е, no apostrophes of transcription. */
export const fold=text=>String(text??'').normalize('NFKD').replace(/\p{M}/gu,'').toLowerCase().replace(/ё/g,'е')
  .replace(/[ʿʾ’'`]/g,'').replace(/[^\p{L}\p{N}]+/gu,' ').trim();

/** The name a member is shown by: the Russian one in Russian, the Latin-script one otherwise. */
export const displayName=(member,locale)=>(locale==='ru'?member.name.ru:member.name.en)||member.name.en||member.name.ru||'';

/** The stand's model: every member with its life, the year it is placed by, its century and its folded names. */
export function populationModel(population,chronology){
  const members=Object.entries(population.members).map(([id,member])=>{
    const phase=lifePhase(id,member,chronology),span=spanOf(phase),anchor=span?(span.from??span.to):null;
    return {id,member,phase,span,anchor,century:centuryOf(anchor),
      haystack:fold([member.name.ru,member.name.en,member.name.original,id].filter(Boolean).join(' '))};
  });
  return {members,byId:new Map(members.map(entry=>[entry.id,entry])),redirects:population.redirects??{},kinds:population.kinds??{}};
}

/** A member by its ID, following the redirects of merged IDs to the member they lead to. */
export function findMember(model,id){
  let at=id;const seen=new Set();
  while(model.redirects[at]&&!seen.has(at)){seen.add(at);at=model.redirects[at];}
  return model.byId.get(at)??null;
}

/** The members a reader's search, kind and century leave; every word of the search must be in a member's names. */
export function filterMembers(model,{query='',kind=null,century=null}={}){
  const terms=fold(query).split(' ').filter(Boolean);
  return model.members.filter(entry=>(!kind||entry.member.kind===kind)&&(century===null||entry.century===century)
    &&terms.every(term=>entry.haystack.includes(term)));
}

/** Members in the reader's order: by name in the reader's language, or by time with the undated last. */
export function sortMembers(list,{by='name',locale='ru'}={}){
  // A register's label may open with a bracket or a quote; the order reads the name, not its punctuation.
  const collator=new Intl.Collator(locale==='zh-Hans'?'zh':locale,{sensitivity:'base',ignorePunctuation:true});
  const byName=(a,b)=>collator.compare(displayName(a.member,locale),displayName(b.member,locale));
  if(by!=='time')return [...list].sort(byName);
  return [...list].sort((a,b)=>(a.anchor===null)-(b.anchor===null)||(a.anchor??0)-(b.anchor??0)||byName(a,b));
}

/** How many members each century holds, and how many of each kind, for the stand's filters. */
export function tally(list){
  const centuries=new Map(),kinds=new Map();
  for(const entry of list){
    if(entry.century!==null)centuries.set(entry.century,(centuries.get(entry.century)??0)+1);
    kinds.set(entry.member.kind,(kinds.get(entry.member.kind)??0)+1);
  }
  return {centuries,kinds};
}

/** The register rows a member grew from, by batch, each batch with the address of its register (population.batches). */
export function rowsByBatch(member,batches={}){
  const out=new Map();
  for(const row of member.grew_from){
    const [batch,record]=row.split(':');
    if(!out.has(batch))out.set(batch,{batch,rows:[],register:batches[batch]?.register??null});
    out.get(batch).rows.push(record);
  }
  return [...out.values()];
}
