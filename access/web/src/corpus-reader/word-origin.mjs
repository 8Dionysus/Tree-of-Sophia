// Where a selected word comes from: what the dictionaries tell of it, its
// descent from the oldest form to the word in the book, the parts it is made
// of, its kin in other languages, the first editions' spelling, where the
// German written before and beside the book uses it, how often the book says
// it, and, for a translation's word, the word of the original it renders
// together with that word's history. Records load only when the reader asks
// for an origin.
import witness from '../../../../bonsai/zarathustra/witnesses.json' with {type:'json'};
import {copyLocale,localeText,resolveLocale} from '../public-research/locale.mjs';
import {languageName} from './language-names.mjs';

// The origins live in R2 with the word layer (bonsai/zarathustra/layers.json); without them no word has one.
const manifest=Object.values(import.meta.glob('../../../../bonsai/zarathustra/origins.json',{eager:true,import:'default'}))[0]??{};
const shardFiles=import.meta.glob('../../../../bonsai/zarathustra/origins/*.json',{import:'default'});
// The histories live in R2 beside them (bonsai/review/philology/histories.py).
const historyManifest=Object.values(import.meta.glob('../../../../bonsai/zarathustra/histories.json',{eager:true,import:'default'}))[0]??{};
const historyFiles=import.meta.glob('../../../../bonsai/zarathustra/histories/*.json',{import:'default'});
const cache=new Map();
const ORIGINAL=manifest.original??'de';

/** FNV-1a 32-bit over UTF-8, the shard key `origins.py` writes with. */
export function shardOf(lemma,count=manifest.shards?.count??32){
  let hash=0x811c9dc5;
  for(const byte of new TextEncoder().encode(lemma)){hash^=byte;hash=Math.imul(hash,0x01000193)>>>0;}
  return hash%count;
}

async function shardRecord(files,language,lemma){
  if(!language||!lemma)return null;
  const name=`${language}-${String(shardOf(lemma)).padStart(2,'0')}`;
  const path=Object.keys(files).find(value=>value.endsWith(`/${name}.json`));
  if(!path)return null;
  if(!cache.has(path))cache.set(path,files[path]().catch(error=>{cache.delete(path);throw error;}));
  const shard=await cache.get(path);
  return shard?.lemmas?.[lemma]??null;
}

/** The stand's origin record of a lemma in a witness language, or null. */
export const originRecord=(language,lemma)=>shardRecord(shardFiles,language,lemma);

/** The stand's history record of a lemma in a witness language, or null. */
export const historyRecord=(language,lemma)=>shardRecord(historyFiles,language,lemma);

const COPY={
  ru:{title:'Происхождение слова',choose:'Выделите одно слово.',loading:'Ищем происхождение…',unknown:'Происхождение этого слова стенд пока не знает.',
    renders:'Переводит слово оригинала',made:'Сложено из',kin:'Родственные слова',first:'Впервые в книге',open:'Открыть',
    editions:'в первых изданиях',today:'сегодня',calque:'калька',borrowed:'заимствовано',derived:'образовано',semantic:'смысловое заимствование',
    wiktionary:'Викисловарь',edition_en:'английский Викисловарь',edition_de:'немецкий Викисловарь',edition_ru:'русский Викисловарь',
    more:'Ещё в Викисловаре',texts:'В немецких текстах',firstText:'Впервые в Немецком текстовом архиве — {0}',
    frequency:'Частота в архиве по четвертям века, {0}–{1}; отмечена четверть «Заратустры»',per:'{0}–{1}: употреблений — {2}; на 10 000 существительных — {3}',
    nietzsche:'У Ницше в архиве',page:'с. {0}',inBook:'В книге — {0}',part:'часть {0}: {1}',archive:'Немецкий текстовый архив'},
  en:{title:'Origin of the word',choose:'Select a single word.',loading:'Looking for the origin…',unknown:'The stand does not yet know where this word comes from.',
    renders:'Translates the word of the original',made:'Made of',kin:'Kindred words',first:'First in the book',open:'Open',
    editions:'in the first editions',today:'today',calque:'calque',borrowed:'borrowed',derived:'formed',semantic:'semantic loan',
    wiktionary:'Wiktionary',edition_en:'English Wiktionary',edition_de:'German Wiktionary',edition_ru:'Russian Wiktionary',
    more:'More in Wiktionary',texts:'In German texts',firstText:'First in the German Text Archive: {0}',
    frequency:'Frequency in the archive by quarter century, {0}–{1}; the quarter of Zarathustra marked',per:'{0}–{1}: {2} uses, {3} per 10,000 nouns',
    nietzsche:"Nietzsche's works in the archive",page:'p. {0}',inBook:'In the book: {0}',part:'part {0}: {1}',archive:'German Text Archive'},
};
const TIMES={ru:{one:'{0} раз',few:'{0} раза',many:'{0} раз',other:'{0} раза'},en:{one:'once',other:'{0} times'}};
const RELATION=new Map([['calque','calque'],['borrowed','borrowed'],['derived','derived'],['semantic loan','semantic']]);
const words=locale=>{
  const en=copyLocale(locale)==='en',base=COPY[en?'en':'ru'];
  return Object.fromEntries(Object.entries(base).map(([key,value])=>[key,en?localeText(value,locale):value]));
};
const fill=(template,...values)=>template.replace(/\{(\d+)\}/g,(match,index)=>index<values.length?String(values[index]):match);
function times(n,locale){
  const russian=copyLocale(locale)==='ru',table=TIMES[russian?'ru':'en'];
  const form=new Intl.PluralRules(russian?'ru':'en').select(n);
  const template=table[form]??table.other;
  return fill(russian?template:localeText(template,locale),n);
}
const el=(tag,className,text)=>{const node=document.createElement(tag);if(className)node.className=className;if(text!=null)node.textContent=text;return node;};
const plain=term=>String(term).normalize('NFD').replace(/[́̀]/gu,'').normalize('NFC');
const chapterTitle=(key,language)=>witness.chapters?.find(item=>item.key===key)?.titles?.[language]??key;

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

const wiktionary=edition=>`https://${edition}.wiktionary.org/wiki/`;
const SECTION={de:'German',ru:'Russian'};
const link=(href,text)=>{const a=el('a','cr-origin-link',text);a.href=href;a.target='_blank';a.rel='noopener';return a;};

/** The editions of Wiktionary in the order the reader reads them: the reader's language, the word's, then English. */
export function toldOrder(told,locale,language){
  const own=resolveLocale(locale);
  return [...new Set([own,copyLocale(locale),language,'en','ru','de'])].filter(edition=>told?.[edition]?.text);
}

function telling(parent,edition,item,copy,language){
  const text=el('p','cr-origin-told',item.text);text.lang=edition;
  const source=el('p','cr-origin-source');
  const href=`${wiktionary(edition)}${encodeURIComponent(item.entry)}${edition==='en'&&SECTION[language]?`#${SECTION[language]}`:''}`;
  source.append(document.createTextNode(`${copy[`edition_${edition}`]??copy.wiktionary} · `),link(href,item.entry));
  // The Russian edition names the dictionaries it draws on; the credit stays in the edition's own words.
  if(item.basis?.length)source.append(document.createTextNode(` · по данным ${item.basis.join('; ')}`));
  parent.append(text,source);
}

/** What the dictionaries tell of the word: the edition the reader reads first, the others folded beneath. */
function told(parent,history,copy,locale,language){
  const order=toldOrder(history?.told,locale,language);
  if(!order.length)return false;
  telling(parent,order[0],history.told[order[0]],copy,language);
  if(order.length>1){
    const more=el('details','cr-origin-more');more.append(el('summary','',copy.more));
    for(const edition of order.slice(1))telling(more,edition,history.told[edition],copy,language);
    parent.append(more);
  }
  return true;
}

/** The quarter centuries the archive holds enough of to read a word's frequency against. */
export function frequencyBins(attested,periods=historyManifest.attested?.periods,nouns=historyManifest.attested?.nouns,least=30000){
  if(!attested?.n||!periods||!nouns)return [];
  const offset=Math.round((attested.from-periods.from)/periods.step);
  return attested.n.map((n,index)=>({from:attested.from+index*periods.step,to:attested.from+(index+1)*periods.step-1,n,nouns:nouns[index+offset]??0}))
    .filter(bin=>bin.nouns>=least).map(bin=>({...bin,rate:bin.n*10000/bin.nouns}));
}

function sparkline(parent,attested,copy,locale){
  const bins=frequencyBins(attested);
  if(bins.length<2||!bins.some(bin=>bin.n))return;
  const NS='http://www.w3.org/2000/svg',width=bins.length*8,height=34,top=Math.max(...bins.map(bin=>bin.rate));
  const svg=document.createElementNS(NS,'svg');
  svg.setAttribute('class','cr-origin-frequency');svg.setAttribute('viewBox',`0 0 ${width} ${height}`);svg.setAttribute('preserveAspectRatio','none');svg.setAttribute('role','img');
  const label=fill(copy.frequency,bins[0].from,bins.at(-1).to+1);svg.setAttribute('aria-label',label);
  const format=new Intl.NumberFormat(resolveLocale(locale),{maximumFractionDigits:2});
  for(const [index,bin] of bins.entries()){
    const h=bin.n?Math.max(1.5,bin.rate/top*(height-2)):0.6,bar=document.createElementNS(NS,'rect');
    bar.setAttribute('x',String(index*8+1));bar.setAttribute('y',String(height-h));bar.setAttribute('width','6');bar.setAttribute('height',String(h));
    bar.setAttribute('class',bin.from<=1883&&1883<=bin.to?'cr-origin-bar cr-origin-bar-book':'cr-origin-bar');
    const title=document.createElementNS(NS,'title');title.textContent=fill(copy.per,bin.from,bin.to,bin.n,format.format(bin.rate));
    bar.append(title);svg.append(bar);
  }
  const figure=el('figure','cr-origin-figure');
  const axis=el('figcaption','cr-origin-axis');axis.append(el('span','',String(bins[0].from)),el('span','',String(bins.at(-1).to+1)));
  figure.append(svg,axis);parent.append(figure);
}

/** Where the German written before and beside the book uses the word: the earliest works, the frequency, Nietzsche's works. */
function attested(parent,history,copy,locale){
  const record=history?.attested;
  if(!record)return false;
  const block=el('div','cr-origin-attested');
  block.append(el('h4','cr-origin-subhead',copy.texts));
  const first=record.first??(record.period?record.period.join('–'):null);
  if(first)block.append(el('p','cr-origin-line',fill(copy.firstText,first)));
  sparkline(block,record,copy,locale);
  if(record.early?.length){
    const list=el('ol','cr-origin-uses');
    for(const [year,author,title,text,span,basename,page,mark] of record.early){
      const row=el('li','cr-origin-use');
      const who=el('p','cr-origin-who');
      who.append(el('span','cr-origin-year',String(year)),document.createTextNode(` ${[author,title].filter(Boolean).join(', ')}`));
      const said=el('q','cr-origin-said');said.lang=ORIGINAL;
      const [start,end]=span??[0,0];
      said.append(document.createTextNode(text.slice(0,start)),el('mark','',text.slice(start,end)),document.createTextNode(text.slice(end)));
      row.append(who,said);
      if(basename){
        const book=historyManifest.attested?.book??'https://www.deutschestextarchiv.de/book/view/{basename}/?p={page}&hl={hl}';
        const href=book.replace('{basename}',encodeURIComponent(basename)).replace('{page}',String(page??1)).replace('{hl}',encodeURIComponent(mark??''));
        const source=el('p','cr-origin-source');source.append(link(href,`${copy.archive}${page?`, ${fill(copy.page,page)}`:''}`));
        row.append(source);
      }
      list.append(row);
    }
    block.append(list);
  }
  if(record.nietzsche?.length)block.append(el('p','cr-origin-line',`${copy.nietzsche}: ${record.nietzsche.map(([year,title])=>`${title}, ${year}`).join(' · ')}`));
  parent.append(block);
  return true;
}

async function wordOrigin(parent,lemma,record,copy,locale,language,heading=null,history=null){
  const block=el('section','cr-origin-word');
  if(heading)block.append(el('h3','cr-origin-heading',heading));
  const shown=[told(block,history,copy,locale,language),descent(block,lemma,record,copy,locale,language),
    await parts(block,record,copy,locale,language),kin(block,record,copy,locale),attested(block,history,copy,locale)];
  if(shown.some(Boolean))parent.append(block);
  return shown.some(Boolean);
}

/** How often the book says the word, part by part. */
function inBook(parent,history,copy,locale){
  const book=history?.book;
  if(!book?.n)return;
  const parts=Object.entries(book.parts??{}).map(([part,n])=>fill(copy.part,part,n));
  parent.append(el('p','cr-origin-book',`${fill(copy.inBook,times(book.n,locale))}${parts.length>1?` · ${parts.join(' · ')}`:''}`));
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
  const [record,history]=await Promise.all([originRecord(lang,lemma).catch(()=>null),historyRecord(lang,lemma).catch(()=>null)]);
  let shown=false;
  // A translation's word comes from the word of the original it renders: that word's history comes first.
  const source=lang!==ORIGINAL?record?.renders?.[0]?.[0]:null;
  if(source){
    const heading=`${copy.renders} · ${source}`;
    const [sourceRecord,sourceHistory]=await Promise.all([originRecord(ORIGINAL,source).catch(()=>null),historyRecord(ORIGINAL,source).catch(()=>null)]);
    if(!(await wordOrigin(body,source,sourceRecord,copy,locale,ORIGINAL,heading,sourceHistory)))body.append(el('h3','cr-origin-heading',heading));
    shown=true;
  }
  shown=(await wordOrigin(body,lemma,record,copy,locale,lang,source?lemma:null,history))||shown;
  status.remove();
  if(!shown)body.append(el('p','cr-philology-muted',copy.unknown));
  inBook(body,history,copy,locale);
  if(record?.first){
    const [chapter,unitId,start,end]=record.first,row=el('p','cr-origin-first');
    row.append(document.createTextNode(`${copy.first}: ${chapterTitle(chapter,lang)} `));
    if(typeof onOpenPlace==='function'){const open=el('button','cr-origin-open',copy.open);open.type='button';open.addEventListener('click',()=>onOpenPlace({chapter,unitId,start,end,language:lang}));row.append(open);}
    body.append(row);
  }
}

/** The parts of a word's history, each into `parent`, as the origin sheet draws them. */
export const renderTold=(parent,history,{locale='ru',language=ORIGINAL}={})=>told(parent,history,words(locale),locale,language);
export const renderAttested=(parent,history,{locale='ru'}={})=>attested(parent,history,words(locale),locale);
export const renderInBook=(parent,history,{locale='ru'}={})=>inBook(parent,history,words(locale),locale);
