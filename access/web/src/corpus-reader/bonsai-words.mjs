// The bonsai's word layer at a selected place of a Zarathustra witness: the
// lemma, part of speech, features, today's spelling and syntactic head of the
// word in its sentence. A machine proposal, unreviewed; it is shown only where
// the reader's unit text is exactly the witness text the layer was made from.
import manifest from '../../../../bonsai/zarathustra/tokens.json' with {type:'json'};
import witness from '../../../../bonsai/zarathustra/witnesses.json' with {type:'json'};
import spelling from '../../../../bonsai/zarathustra/spelling.json' with {type:'json'};
import {unitWords,wordsAt,sourceForm} from '../../../../bonsai/lib/tokens.mjs';

const tokenFiles=import.meta.glob('../../../../bonsai/zarathustra/tokens/*.json',{import:'default'});
const witnessFiles=import.meta.glob('../../../../bonsai/zarathustra/witnesses/*.json',{import:'default'});
// The reader version of each witness: the corpus key of witnesses.json and the witness language.
const LANGUAGES=[['dta','de'],['ru','ru'],['en','en']];
const loaded=new Map();
const load=(files,key)=>{
  const path=Object.keys(files).find(value=>value.endsWith(`/${key}.json`));
  if(!path)return Promise.resolve(null);
  if(!loaded.has(path))loaded.set(path,files[path]().catch(error=>{loaded.delete(path);throw error;}));
  return loaded.get(path);
};

/** The witness language a reader version stands for in the bonsai, or null. */
export function bonsaiLanguage(versionId){
  for(const [key,language] of LANGUAGES)if(witness.corpus?.[key]?.versionId===versionId)return language;
  return null;
}

/** The chapter of a unit, from the layer's index of where each chapter begins in the reader's unit order. */
export function chapterOf(version,language,unitId){
  const order=new Map((version?.units??[]).map((unit,index)=>[unit.id,index]));
  const position=order.get(unitId);
  if(position===undefined)return null;
  let found=null;
  for(const [chapter,first] of manifest.index?.[language]??[]){
    const at=order.get(first);
    if(at!==undefined&&at<=position)found=chapter;
  }
  return found;
}

/** Today's spelling the stand reads for a historical form of the original, with its ground, or null. */
export function modernSpelling(form){
  const entry=spelling.forms?.[sourceForm(form)];
  return entry?{...entry,form:sourceForm(form)}:null;
}

/**
 * The words at a selection in a bonsai witness, or null where the layer has none for this version or place.
 * `unitText` is the reader's exact unit text; a unit whose text differs from the witness gets nothing.
 */
export async function bonsaiWords({version,reference,unitText}){
  const language=bonsaiLanguage(version?.id);
  if(!language)return null;
  const chapter=chapterOf(version,language,reference.unitId);
  if(!chapter)return null;
  const [layer,saved]=await Promise.all([load(tokenFiles,chapter),load(witnessFiles,chapter)]);
  const unit=saved?.units?.[language]?.find(item=>item[0]===reference.unitId);
  if(!layer||!unit||unit[2]!==unitText)return null;
  const {start,end}=reference.selector;
  const all=unitWords(layer,language,unit[0],unit[2]);
  const words=wordsAt(layer,language,unit[0],unit[2],start,end).map(word=>{
    const head=word.head>=0?all[word.head]:null;
    return {...word,form:sourceForm(word.text),headLemma:head?.lemma??null,headText:head?sourceForm(head.text):null,
      spelling:language==='de'?modernSpelling(word.text):null};
  });
  if(!words.length)return null;
  return {status:'proposed',language,chapter,words,
    analyzer:{label:{ru:'Разбор слов стенда',en:"The stand's word analysis"},stanza:manifest.analyzer?.stanza??null,basis:manifest.basis},
    posture:manifest.posture};
}

/** Every place of a lemma in one chapter of a witness, for a reader to open. */
export async function lemmaInChapter(language,chapter,lemma){
  const layer=await load(tokenFiles,chapter);
  if(!layer)return [];
  const out=[];
  for(const [unitId,rows] of Object.entries(layer.witnesses?.[language]?.units??{}))
    for(const row of rows)if(layer.entries[row[2]]?.[0]===lemma)out.push({unitId,start:row[0],end:row[1]});
  return out;
}

/** The reader version a witness language stands for. */
export function versionIdOf(language){
  const key=LANGUAGES.find(([,value])=>value===language)?.[0];
  return key?witness.corpus?.[key]?.versionId??null:null;
}

/** The exact text of a span of a witness unit, or null where the chapter or unit is unknown. */
export async function witnessUnitText(chapter,language,unitId,start,end){
  const saved=await load(witnessFiles,chapter);
  const unit=saved?.units?.[language]?.find(item=>item[0]===unitId);
  return unit?Array.from(unit[2]).slice(start,end).join(''):null;
}

/**
 * The word layer's reading of a word selected by its text in a witness unit: the `nth` word (from 0) of the unit
 * whose source form is `form`. A stand that shows a unit in its own display spelling finds the word this way.
 */
export async function bonsaiWordByForm({chapter,language,unitId,form,nth=0,near=null}){
  const [layer,saved]=await Promise.all([load(tokenFiles,chapter),load(witnessFiles,chapter)]);
  const unit=saved?.units?.[language]?.find(item=>item[0]===unitId);
  if(!layer||!unit)return null;
  const all=unitWords(layer,language,unit[0],unit[2]);
  const wanted=sourceForm(form).toLowerCase();
  const matches=all.filter(word=>word.upos!=='PUNCT'&&sourceForm(word.text).toLowerCase()===wanted);
  // A display position, where the stand gives one, finds the nearest occurrence; else the nth.
  const word=Number.isFinite(near)&&matches.length?matches.reduce((a,b)=>Math.abs(b.start-near)<Math.abs(a.start-near)?b:a)
    :matches[Math.min(nth,matches.length-1)];
  if(!word)return null;
  const head=word.head>=0?all[word.head]:null;
  return {status:'proposed',language,chapter,
    words:[{...word,form:sourceForm(word.text),headLemma:head?.lemma??null,headText:head?sourceForm(head.text):null,
      spelling:language==='de'?modernSpelling(word.text):null}],
    analyzer:{label:{ru:'Разбор слов стенда',en:"The stand's word analysis"},stanza:manifest.analyzer?.stanza??null,basis:manifest.basis},
    posture:manifest.posture};
}
