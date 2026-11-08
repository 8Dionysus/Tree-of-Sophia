// Where a selected word comes from: its descent from the oldest form to the
// word in the book, the parts it is made of, its kin in other languages, the
// first editions' spelling, and, for a translation's word, the word of the
// original it renders together with that word's descent. Records load only
// when the reader asks for an origin.
import witness from '../../../../bonsai/zarathustra/witnesses.json' with {type:'json'};
import {copyLocale,localeText} from '../public-research/locale.mjs';
import {chapterTitle} from '../public-research/chapter-title.mjs';
import {languageName} from './language-names.mjs';

// The origins live in R2 with the word layer (bonsai/zarathustra/layers.json); without them no word has one.
const manifest=Object.values(import.meta.glob('../../../../bonsai/zarathustra/origins.json',{eager:true,import:'default'}))[0]??{};
const shardFiles=import.meta.glob('../../../../bonsai/zarathustra/origins/*.json',{import:'default'});
const cache=new Map();
const ORIGINAL=manifest.original??'de';

/** FNV-1a 32-bit over UTF-8, the shard key `origins.py` writes with. */
export function shardOf(lemma,count=manifest.shards?.count??32){
  let hash=0x811c9dc5;
  for(const byte of new TextEncoder().encode(lemma)){hash^=byte;hash=Math.imul(hash,0x01000193)>>>0;}
  return hash%count;
}

/** The stand's origin record of a lemma in a witness language, or null. */
export async function originRecord(language,lemma){
  if(!language||!lemma)return null;
  const name=`${language}-${String(shardOf(lemma)).padStart(2,'0')}`;
  const path=Object.keys(shardFiles).find(value=>value.endsWith(`/${name}.json`));
  if(!path)return null;
  if(!cache.has(path))cache.set(path,shardFiles[path]().catch(error=>{cache.delete(path);throw error;}));
  const shard=await cache.get(path);
  return shard?.lemmas?.[lemma]??null;
}

const COPY={
  ru:{title:'Происхождение слова',choose:'Выделите одно слово.',loading:'Ищем происхождение…',unknown:'Происхождение этого слова стенд пока не знает.',
    renders:'Переводит слово оригинала',made:'Сложено из',kin:'Родственные слова',first:'Впервые в книге',open:'Открыть',
    editions:'в первых изданиях',today:'сегодня',calque:'калька',borrowed:'заимствовано',derived:'образовано',semantic:'смысловое заимствование'},
  en:{title:'Origin of the word',choose:'Select a single word.',loading:'Looking for the origin…',unknown:'The stand does not yet know where this word comes from.',
    renders:'Translates the word of the original',made:'Made of',kin:'Kindred words',first:'First in the book',open:'Open',
    editions:'in the first editions',today:'today',calque:'calque',borrowed:'borrowed',derived:'formed',semantic:'semantic loan'},
};
const RELATION=new Map([['calque','calque'],['borrowed','borrowed'],['derived','derived'],['semantic loan','semantic']]);
const words=locale=>{
  const en=copyLocale(locale)==='en',base=COPY[en?'en':'ru'];
  return Object.fromEntries(Object.entries(base).map(([key,value])=>[key,en?localeText(value,locale):value]));
};
const el=(tag,className,text)=>{const node=document.createElement(tag);if(className)node.className=className;if(text!=null)node.textContent=text;return node;};
const plain=term=>String(term).normalize('NFD').replace(/[́̀]/gu,'').normalize('NFC');
// The chapter as the word's witness titles it, else as the original does.
const chapterName=(key,language)=>{const chapter=witness.chapters?.find(item=>item.key===key);return chapterTitle(chapter,language)||chapterTitle(chapter,ORIGINAL)||key;};

/** One generation of the descent: the language, the form, and how it passed on where that is no plain inheritance. */
function step(list,{language,term,relation}){
  const row=el('li','cr-origin-step');
  row.append(el('span','cr-origin-language',language),el('span','cr-origin-form',term));
  if(relation)row.append(el('span','cr-origin-relation',relation));
  list.append(row);
}

/** The descent of a lemma, oldest form first, down to the word itself as the book writes it. */
function descent(parent,lemma,record,copy,locale,language){
  const russian=copyLocale(locale)==='ru';
  const list=el('ol','cr-origin-descent');
  for(const [relation,code,term] of record?.origin?.[0]?.chain??[])
    step(list,{language:languageName(code,russian)??code,term,relation:RELATION.has(relation)?copy[RELATION.get(relation)]:null});
  if(list.childElementCount){
    if(record?.spelling?.length)step(list,{language:copy.editions,term:record.spelling.slice(0,3).join(', ')});
    step(list,{language:record?.spelling?.length?copy.today:languageName(language,russian)??language,term:lemma});
    parent.append(list);
    return true;
  }
  if(record?.spelling?.length){
    const row=el('p','cr-origin-line');
    row.append(document.createTextNode(`${copy.editions}: `),el('em','',record.spelling.slice(0,3).join(', ')),document.createTextNode(` · ${copy.today}: `),el('em','',lemma));
    parent.append(row);
    return true;
  }
  return false;
}

/** The parts a word is made of, each with the oldest form its own descent reaches. */
async function parts(parent,record,copy,locale,language){
  const items=record?.origin?.[0]?.parts;
  if(!items?.length)return false;
  const russian=copyLocale(locale)==='ru';
  const row=el('p','cr-origin-line');
  row.append(document.createTextNode(`${copy.made}: `));
  for(const [index,part] of items.entries()){
    if(index)row.append(document.createTextNode(' + '));
    row.append(el('em','',plain(part)));
    const own=await originRecord(language,plain(part).replace(/^-|-$/gu,'')).catch(()=>null);
    const oldest=own?.origin?.[0]?.chain?.[0];
    if(oldest)row.append(document.createTextNode(` (${languageName(oldest[1],russian)??oldest[1]} ${oldest[2]})`));
  }
  parent.append(row);
  return true;
}

function kin(parent,record,copy,locale){
  const russian=copyLocale(locale)==='ru';
  const known=(record?.origin?.[0]?.cognates??[]).map(([code,term])=>[languageName(code,russian),term]).filter(([name])=>name).slice(0,6);
  if(!known.length)return false;
  parent.append(el('p','cr-origin-line',`${copy.kin}: ${known.map(([name,term])=>`${name} ${term}`).join(', ')}`));
  return true;
}

async function wordOrigin(parent,lemma,record,copy,locale,language,heading=null){
  const block=el('section','cr-origin-word');
  if(heading)block.append(el('h3','cr-origin-heading',heading));
  const shown=[descent(block,lemma,record,copy,locale,language),await parts(block,record,copy,locale,language),kin(block,record,copy,locale)];
  if(shown.some(Boolean))parent.append(block);
  return shown.some(Boolean);
}

/**
 * Render the origin of the selected word into `body`. `context` is the word layer's reading at the place,
 * `language` the witness language; `onOpenPlace({chapter, unitId, start, end, language})` opens a place of the book.
 */
export async function renderOrigin(body,{locale='ru',form='',language='',context=null,onOpenPlace}={}){
  const copy=words(locale);
  body.append(el('h2','cr-inspector-title',copy.title));
  body.append(el('blockquote','cr-philology-form',form||copy.choose));
  const word=context?.words?.find(item=>['NOUN','PROPN','VERB','ADJ','ADV'].includes(item.upos))??context?.words?.[0]??null;
  const lang=context?.language??language;
  const lemma=word?.lemma??form;
  if(!lemma)return;
  const status=el('p','cr-philology-muted',copy.loading);body.append(status);
  const record=await originRecord(lang,lemma).catch(()=>null);
  let shown=false;
  // A translation's word comes from the word of the original it renders: that word's descent comes first.
  const source=lang!==ORIGINAL?record?.renders?.[0]?.[0]:null;
  if(source){
    const heading=`${copy.renders} · ${source}`;
    const sourceRecord=await originRecord(ORIGINAL,source).catch(()=>null);
    if(!(await wordOrigin(body,source,sourceRecord,copy,locale,ORIGINAL,heading)))body.append(el('h3','cr-origin-heading',heading));
    shown=true;
  }
  shown=(await wordOrigin(body,lemma,record,copy,locale,lang,source?lemma:null))||shown;
  status.remove();
  if(!shown)body.append(el('p','cr-philology-muted',copy.unknown));
  if(record?.first){
    const [chapter,unitId,start,end]=record.first,row=el('p','cr-origin-first');
    row.append(document.createTextNode(`${copy.first}: ${chapterName(chapter,lang)} `));
    if(typeof onOpenPlace==='function'){const open=el('button','cr-origin-open',copy.open);open.type='button';open.addEventListener('click',()=>onOpenPlace({chapter,unitId,start,end,language:lang}));row.append(open);}
    body.append(row);
  }
}
