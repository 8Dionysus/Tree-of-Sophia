// The philological core of a bonsai: the words that carry its work, each kept with
// the word of the original, the place where the book first says it, the word each
// translation shown beside it uses there, and the word the Tree itself reads in
// every language. A reader's text names a term with the Tree's word; the term
// opens on its origin: the original word, the translations' words, the first place.
//
// The words of a translation are checked against the witnesses: at the first place
// the translation's text holds one of the forms the term gives for it. The Tree's
// own words are a choice of the stand and are kept as given.

import {citeIssues,fragmentOf,readChapter,readingText} from './witnesses.mjs';

export const TERMS_SCHEMA='tos.bonsai.terms.v1';
const LANGUAGES=['ru','en'];
const ID=/^[a-z0-9]+(?:-[a-z0-9]+)*$/;

const words=value=>Array.isArray(value)&&value.length>0&&value.every(item=>typeof item==='string'&&item.trim()===item&&item.length>0);
const text=value=>typeof value==='string'&&value.trim().length>0;
const escape=value=>value.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');

/** A pattern that finds any of the forms as whole words, ignoring case. */
export function formPattern(forms){
  const sorted=[...new Set(forms)].sort((a,b)=>b.length-a.length).map(escape);
  return new RegExp(`(?<![\\p{L}\\p{N}])(?:${sorted.join('|')})(?![\\p{L}\\p{N}])`,'iu');
}

/** Whether a text holds one of the forms as a whole word. */
export const holdsForm=(value,forms)=>formPattern(forms).test(String(value??'').replace(/\s+/g,' '));

// Greek accents and breathings differ between editions. Keep the original text
// for display, but compare Greek spellings without those marks or final sigma.
export const greekKey=value=>String(value??'').normalize('NFD').replace(/\p{M}/gu,'').toLowerCase().replaceAll('ς','σ');
const holdsOriginal=(place,forms,language)=>{
  const key=language==='grc'?greekKey:value=>String(value??'').toLowerCase();
  const before=key(place.before??' '),value=key(place.text),after=key(place.after??' ');
  const pattern=new RegExp(formPattern(forms.map(key)).source,'giu');
  return [...(before+value+after).matchAll(pattern)].some(match=>match.index>=before.length&&match.index+match[0].length<=before.length+value.length);
};

/** Problems of a terms module, its first places checked against the witness chapters (by key). */
export function termsIssues(module,chapterFiles={},{pivot='de',place=null}={}){
  const issues=[];
  if(module?.schema!==TERMS_SCHEMA)issues.push('schema');
  if(!Array.isArray(module?.terms)||!module.terms.length){issues.push('no terms');return issues;}
  const editions=module.editions??{};
  for(const [id,edition] of Object.entries(editions))if(!LANGUAGES.includes(edition?.lang)||!text(edition?.label?.ru)||!text(edition?.label?.en))issues.push(`edition ${id}: lang and label`);
  const chapters=new Map(),seen=new Set(),claimed=new Map();
  const chapterOf=key=>{if(!chapters.has(key))chapters.set(key,chapterFiles[key]?readChapter(chapterFiles[key]):null);return chapters.get(key);};
  for(const term of module.terms){
    const at=`term ${term?.id??'?'}`;
    if(!ID.test(term?.id??'')){issues.push(`${at}: id`);continue;}
    if(seen.has(term.id))issues.push(`${at}: repeated`);seen.add(term.id);
    const original=term.original??{};
    if(!(Array.isArray(pivot)?pivot.includes(original.lang):original.lang===pivot)||!text(original.lemma)||!words(original.forms)||!original.forms.includes(original.lemma))issues.push(`${at}: original lang, lemma and forms (with the lemma)`);
    if(place)for(const form of original.forms??[]){
      const key=`${original.lang}:${original.lang==='grc'?greekKey(form):form.toLowerCase()}`;
      if(claimed.has(key)&&claimed.get(key)!==term.id)issues.push(`${at}: original form "${form}" belongs to ${claimed.get(key)}`);
      claimed.set(key,term.id);
    }
    for(const language of LANGUAGES){
      const rendering=term.renderings?.[language]??{},tos=rendering.tos??{};
      if(!text(tos.term)||!words(tos.forms)||!tos.forms.includes(tos.term))issues.push(`${at}: the Tree's ${language} term and its forms (with the term)`);
      for(const witness of [].concat(rendering.witness??[])){
        if(witness.edition&&(!editions[witness.edition]||editions[witness.edition].lang!==language))issues.push(`${at}: ${language} witness edition ${witness.edition}`);
        if(witness.edition&&(!text(witness.term)||!words(witness.forms)))issues.push(`${at}: the ${language} witness's term and forms`);
      }
      if(rendering.avoid!==undefined&&!words(rendering.avoid))issues.push(`${at}: ${language} avoid`);
      // A form the Tree reads may not be one it avoids, and two terms may not claim the same form.
      for(const form of tos.forms??[]){
        if((rendering.avoid??[]).some(item=>item.toLowerCase()===form.toLowerCase()))issues.push(`${at}: ${language} form "${form}" is also avoided`);
        const key=`${language}:${form.toLowerCase()}`;if(claimed.has(key)&&claimed.get(key)!==term.id)issues.push(`${at}: ${language} form "${form}" belongs to ${claimed.get(key)}`);claimed.set(key,term.id);
      }
    }
    if(!text(term.gloss?.ru)||!text(term.gloss?.en))issues.push(`${at}: gloss`);
    const cite=term.first;
    if(!cite?.c&&place){
      const result=place(cite,original.lang);
      if(result?.error)issues.push(`${at}: first place ${result.error}`);
      else if(!result||!holdsOriginal(result,original.forms??[],original.lang))issues.push(`${at}: the original does not say ${original.lemma} at ${cite?.w??'?'}`);
      if(result&&!result.error&&cite?.q&&!result.text.includes(cite.q))issues.push(`${at}: first place quote is not in the cited original`);
      continue;
    }
    if(!cite?.c||!Array.isArray(cite.u)){issues.push(`${at}: first place`);continue;}
    const chapter=chapterOf(cite.c);
    if(!chapter){issues.push(`${at}: no witness chapter ${cite.c}`);continue;}
    const problems=citeIssues(cite,chapter,{pivot});
    if(problems.length){issues.push(...problems.map(problem=>`${at}: first place ${problem}`));continue;}
    const originalUnits=fragmentOf(cite,chapter,pivot,{pivot})?.units??[];
    const said=originalUnits.map(unit=>unit.text.slice(unit.from,unit.to)).join(' ');
    if(!text(cite.q)||!readingText(said,'p').includes(readingText(cite.q,'p')))issues.push(`${at}: first place quote is not in the cited original`);
    // Keep the characters just outside the cited span when checking word boundaries.
    // Otherwise a slice such as "esel" inside "geselltest" becomes a false word.
    const firstUnit=originalUnits[0],lastUnit=originalUnits.at(-1);
    const before=firstUnit?.from>0?firstUnit.text[firstUnit.from-1]:' ';
    const after=lastUnit?.to<lastUnit?.text.length?lastUnit.text[lastUnit.to]:' ';
    const context=before+said+after;
    const pattern=original.forms?new RegExp(formPattern(original.forms).source,'giu'):null;
    const inPlace=pattern&&[...context.matchAll(pattern)].some(match=>match.index>=before.length&&match.index+match[0].length<=before.length+said.length);
    if(original.forms&&!inPlace)issues.push(`${at}: the original does not say ${original.lemma} at ${cite.c}`);
    for(const language of LANGUAGES){
      const witness=term.renderings?.[language]?.witness;
      if(!witness?.edition||!words(witness.forms))continue;
      const fragment=fragmentOf(cite,chapter,language,{pivot});
      if(fragment?.omitted)continue;
      // The counterpart may run past the place; the whole counterpart units are read.
      const whole=(fragment?.units??[]).map(unit=>unit.text).join(' ');
      if(!holdsForm(whole,witness.forms))issues.push(`${at}: ${witness.edition} does not say ${witness.term} at ${cite.c}`);
    }
  }
  return issues;
}
