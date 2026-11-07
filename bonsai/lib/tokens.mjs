// The word layer of a bonsai: every word of every witness at its exact place,
// with today's spelling, lemma, part of speech, features and syntactic head.
// `bonsai/review/philology/annotate.py` writes it; this module reads and checks
// it, in the check and in the browser alike (no Node imports).
//
// A chapter file keeps, per witness language, a digest of the witness units it
// was made from and, per unit, rows [start, end, entry, head, deprel, basis,
// reading?]. Offsets count the Unicode code points of the unit's exact text; a
// word broken at a line end spans the break. `entry` points into the chapter's
// shared list [lemma, upos, feats] (Universal Dependencies part of speech and features). `head` is the row index of the
// syntactic head in the same unit, or -1 for a sentence root. `basis` says how
// the lemma stands: `a` model and Wiktionary agree, `l` Wiktionary's lemma
// replaces a model lemma that is no word, `m` the model alone. `reading` is the
// word as the analyzer read it, where it differs from the source's letters
// (today's spelling, a joined line break, the second word of "zum").

export const TOKENS_SCHEMA='tos.bonsai.tokens.v1';
export const TOKENS_CHAPTER_SCHEMA='tos.bonsai.tokens.chapter.v1';
export const SPELLING_SCHEMA='tos.bonsai.spelling.v1';
const BASES=new Set(['a','l','m']);
const UPOS=new Set(['ADJ','ADP','ADV','AUX','CCONJ','DET','INTJ','NOUN','NUM','PART','PRON','PROPN','PUNCT','SCONJ','SYM','VERB','X']);

const cps=text=>Array.from(String(text??''));

/** The digest the word layer records for one witness's units of a chapter; `sha256(text)` gives hex. */
export function unitsDigest(units,sha256){
  return `sha256:${sha256(JSON.stringify(units.map(unit=>[unit[0],unit[2]])))}`;
}

/** The words of a unit as objects, each with its exact source text. */
export function unitWords(chapter,language,unitId,unitText){
  const rows=chapter?.witnesses?.[language]?.units?.[unitId];
  if(!Array.isArray(rows))return [];
  const chars=unitText==null?null:cps(unitText);
  return rows.map((row,index)=>{
    const [start,end,entry,head,deprel,basis,reading]=row,[lemma,upos,feats]=chapter.entries[entry]??[];
    return {index,start,end,lemma,upos,feats,head,deprel,basis,reading:reading??null,
      text:chars?chars.slice(start,end).join(''):null};
  });
}

/** The words a selection [start, end) covers: those that overlap it, punctuation aside. */
export function wordsAt(chapter,language,unitId,unitText,start,end){
  return unitWords(chapter,language,unitId,unitText).filter(word=>word.upos!=='PUNCT'&&word.start<end&&word.end>start);
}

/** Every unit of a chapter where a lemma occurs, with its places, in the witness order. */
export function lemmaPlaces(chapter,language,lemma){
  const out=[];
  for(const [unitId,rows] of Object.entries(chapter?.witnesses?.[language]?.units??{})){
    for(const row of rows)if(chapter.entries[row[2]]?.[0]===lemma)out.push({unitId,start:row[0],end:row[1]});
  }
  return out;
}

/** Where a chapter's word layer does not fit its witnesses or its own grammar. */
export function tokenChapterIssues(chapter,witnessChapter,sha256){
  const issues=[],key=witnessChapter?.chapter??'?';
  if(chapter?.schema!==TOKENS_CHAPTER_SCHEMA)return [`${key}: schema`];
  if(chapter.chapter!==witnessChapter.chapter)issues.push(`${key}: chapter ${chapter.chapter}`);
  const entries=Array.isArray(chapter.entries)?chapter.entries:[];
  entries.forEach((entry,index)=>{if(!Array.isArray(entry)||typeof entry[0]!=='string'||!UPOS.has(entry[1]))issues.push(`${key}: entry ${index}`);});
  for(const [language,units] of Object.entries(witnessChapter.units??{}))
    if(Array.isArray(units)&&units.length&&!chapter.witnesses?.[language])issues.push(`${key} ${language}: no words for this witness`);
  for(const [language,layer] of Object.entries(chapter.witnesses??{})){
    const units=witnessChapter.units?.[language];
    if(!Array.isArray(units)){issues.push(`${key} ${language}: no such witness`);continue;}
    const digest=unitsDigest(units,sha256);
    if(layer.digest!==digest){issues.push(`${key} ${language}: the witness changed since its words were analysed; annotate again`);continue;}
    for(const [unitId,,text] of units){
      const rows=layer.units?.[unitId];
      if(!Array.isArray(rows)){issues.push(`${key} ${language} ${unitId}: no words`);continue;}
      const length=cps(text).length;
      rows.forEach((row,index)=>{
        const [start,end,entry,head,deprel,basis]=row;
        if(!Number.isInteger(start)||!Number.isInteger(end)||start<0||end>length||start>=end)issues.push(`${key} ${language} ${unitId} #${index}: span ${start}-${end}`);
        if(!entries[entry])issues.push(`${key} ${language} ${unitId} #${index}: entry ${entry}`);
        if(!Number.isInteger(head)||head<-1||head>=rows.length)issues.push(`${key} ${language} ${unitId} #${index}: head ${head}`);
        if(typeof deprel!=='string'||!BASES.has(basis))issues.push(`${key} ${language} ${unitId} #${index}: relation or basis`);
      });
    }
    for(const unitId of Object.keys(layer.units??{}))if(!units.some(unit=>unit[0]===unitId))issues.push(`${key} ${language}: words for unknown unit ${unitId}`);
  }
  return issues;
}

/** Where the spelling module breaks its shape. */
export function spellingIssues(spelling){
  if(spelling?.schema!==SPELLING_SCHEMA)return ['schema'];
  const issues=[];
  for(const [form,entry] of Object.entries(spelling.forms??{})){
    if(typeof entry?.modern!=='string'||!entry.modern||entry.modern===form)issues.push(`${form}: today's spelling`);
    if(!['wiktionary','rules','rules+compound'].includes(entry?.basis))issues.push(`${form}: basis`);
    if(entry?.basis!=='wiktionary'&&!(entry?.rules??[]).every(rule=>spelling.rules?.[rule]))issues.push(`${form}: unknown rule`);
    if(!Number.isInteger(entry?.count)||entry.count<1)issues.push(`${form}: count`);
  }
  return issues;
}

/** The source's own spelling of a word, read whole across a line-end break. */
export const sourceForm=text=>String(text).replace(/¬\s*/gu,'').replace(/\s+/gu,' ');

/**
 * Forms of each single-word term that the word layer finds under the term's lemma in the original and the term
 * does not list, with how often each occurs: { termId: { form: count } }.
 */
export function termFormGaps(terms,tokenChapters,witnessChapters,spelling=null){
  const wanted=new Map(),gaps={};
  for(const term of terms?.terms??[]){
    const lemma=term.original?.lemma;
    // A term spelled as the first editions spell it (Thier) is found under today's lemma (Tier).
    if(typeof lemma==='string'&&!/[\s-]/u.test(lemma))wanted.set(spelling?.forms?.[lemma]?.modern??lemma,term);
  }
  const language=terms?.terms?.[0]?.original?.lang;
  for(const [key,chapter] of Object.entries(tokenChapters)){
    const units=witnessChapters[key]?.units?.[language]??[];
    for(const [unitId,,text] of units){
      for(const word of unitWords(chapter,language,unitId,text)){
        const term=wanted.get(word.lemma);if(!term)continue;
        const form=sourceForm(word.text);
        if(term.original.forms.includes(form))continue;
        (gaps[term.id]??={})[form]=(gaps[term.id][form]??0)+1;
      }
    }
  }
  return gaps;
}

/** Where the renderings module names a place its witnesses do not hold. */
export function renderingsIssues(renderings,witnessChapters){
  if(renderings?.schema!=='tos.bonsai.renderings.v1')return ['schema'];
  const issues=[];
  for(const [id,entry] of Object.entries(renderings.terms??{}))for(const [language,rendering] of Object.entries(entry)){
    if(!rendering||typeof rendering!=='object'||!Array.isArray(rendering.words))continue;
    const places=[...rendering.words.flatMap(([,list])=>list),...(rendering.other??[])];
    for(const [chapter,unitId,start,end] of places){
      const unit=witnessChapters[chapter]?.units?.de?.find(item=>item[0]===unitId);
      if(!unit||!(start>=0&&end>start&&end<=unit[2].length))issues.push(`${id} ${language}: no place ${chapter} ${unitId} ${start}-${end}`);
    }
  }
  return issues;
}
