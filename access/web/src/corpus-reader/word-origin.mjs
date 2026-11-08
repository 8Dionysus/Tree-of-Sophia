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
// The stories of the terms' words live in R2 beside them (bonsai/review/philology/stories.py).
const storyFiles=Object.values(import.meta.glob('../../../../bonsai/zarathustra/stories.json',{import:'default'}));
const cache=new Map();
const ORIGINAL=manifest.original??'de';
let storyBook=null;

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

/** The story of an original's lemma, told from its sources with every sentence marked, or null. */
export async function storyRecord(lemma){
  if(!lemma||!storyFiles.length)return null;
  storyBook??=storyFiles[0]().catch(error=>{storyBook=null;throw error;});
  return (await storyBook)?.stories?.[lemma]??null;
}

const COPY={
  ru:{title:'Происхождение слова',choose:'Выделите одно слово.',loading:'Ищем происхождение…',unknown:'Происхождение этого слова стенд пока не знает.',
    renders:'Переводит слово оригинала',made:'Сложено из',kin:'Родственные слова',first:'Впервые в книге',open:'Открыть',
    editions:'в первых изданиях',today:'сегодня',calque:'калька',borrowed:'заимствовано',derived:'образовано',semantic:'смысловое заимствование',
    wiktionary:'Викисловарь',edition_en:'английский Викисловарь',edition_de:'немецкий Викисловарь',edition_ru:'русский Викисловарь',
    more:'Ещё в Викисловаре',texts:'В немецких текстах',firstText:'Впервые в Немецком текстовом архиве — {0}',
    frequency:'Частота в архиве по четвертям века, {0}–{1}; отмечена четверть «Заратустры»',per:'{0}–{1}: употреблений — {2}; на 10 000 существительных — {3}',
    nietzsche:'У Ницше в архиве',page:'с. {0}',inBook:'В книге — {0}',part:'часть {0}: {1}',archive:'Немецкий текстовый архив',
    storyNote:'История собрана моделью из источников этого листа; проверка сверила её числа, цитаты и имена, вторая модель её вычитала. Номер у предложения ведёт к его источнику.',
    storyMark:'Источник {0}',storyPieces:'Места книги и слова переводчиков',antonovsky:'Антоновский',common:'Коммон',tree:'Древо',firstPlace:'на первом месте',treeReads:'Древо'},
  en:{title:'Origin of the word',choose:'Select a single word.',loading:'Looking for the origin…',unknown:'The stand does not yet know where this word comes from.',
    renders:'Translates the word of the original',made:'Made of',kin:'Kindred words',first:'First in the book',open:'Open',
    editions:'in the first editions',today:'today',calque:'calque',borrowed:'borrowed',derived:'formed',semantic:'semantic loan',
    wiktionary:'Wiktionary',edition_en:'English Wiktionary',edition_de:'German Wiktionary',edition_ru:'Russian Wiktionary',
    more:'More in Wiktionary',texts:'In German texts',firstText:'First in the German Text Archive: {0}',
    frequency:'Frequency in the archive by quarter century, {0}–{1}; the quarter of Zarathustra marked',per:'{0}–{1}: {2} uses, {3} per 10,000 nouns',
    nietzsche:"Nietzsche's works in the archive",page:'p. {0}',inBook:'In the book: {0}',part:'part {0}: {1}',archive:'German Text Archive',
    storyNote:'A model told this story from the sources of this sheet; a check matched its numbers, quotations and names, and a second model read it. A sentence’s number leads to its source.',
    storyMark:'Source {0}',storyPieces:'Places of the book and the translators’ words',antonovsky:'Antonovsky',common:'Common',tree:'the Tree',firstPlace:'at the first place',treeReads:'The Tree'},
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
  if(language===ORIGINAL)list.dataset.storySource='E';
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
  return [...new Set([resolveLocale(locale),language,'en','ru','de'])].filter(edition=>told?.[edition]?.text);
}

function telling(parent,edition,item,copy,language,tag=null){
  const text=el('p','cr-origin-told',item.text);text.lang=edition;
  if(tag)text.dataset.storySource=`${tag}.${edition}`;
  const source=el('p','cr-origin-source');
  const href=`${wiktionary(edition)}${encodeURIComponent(item.entry)}${edition==='en'&&SECTION[language]?`#${SECTION[language]}`:''}`;
  source.append(document.createTextNode(`${copy[`edition_${edition}`]??copy.wiktionary} · `),link(href,item.entry));
  // The Russian edition names the dictionaries it draws on; the credit stays in the edition's own words.
  if(item.basis?.length)source.append(document.createTextNode(` · по данным ${item.basis.join('; ')}`));
  parent.append(text,source);
}

/** What the dictionaries tell of the word: the edition the reader reads first, the others folded beneath. */
function told(parent,history,copy,locale,language,tag=null){
  const order=toldOrder(history?.told,locale,language);
  if(!order.length)return false;
  telling(parent,order[0],history.told[order[0]],copy,language,tag);
  if(order.length>1){
    const more=el('details','cr-origin-more');more.append(el('summary','',copy.more));
    for(const edition of order.slice(1))telling(more,edition,history.told[edition],copy,language,tag);
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
  const frequency=el('div','cr-origin-frequency-block');frequency.dataset.storySource='A';
  if(first)frequency.append(el('p','cr-origin-line',fill(copy.firstText,first)));
  sparkline(frequency,record,copy,locale);
  block.append(frequency);
  if(record.early?.length){
    const list=el('ol','cr-origin-uses');
    for(const [index,[year,author,title,text,span,basename,page,mark]] of record.early.entries()){
      const row=el('li','cr-origin-use');row.dataset.storySource=`A${index+1}`;
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
  if(record.nietzsche?.length){
    const line=el('p','cr-origin-line',`${copy.nietzsche}: ${record.nietzsche.map(([year,title])=>`${title}, ${year}`).join(' · ')}`);
    line.dataset.storySource='N';block.append(line);
  }
  parent.append(block);
  return true;
}

async function wordOrigin(parent,lemma,record,copy,locale,language,heading=null,history=null,story=null,counted=false){
  const block=el('section','cr-origin-word');
  if(heading)block.append(el('h3','cr-origin-heading',heading));
  const told_=story?renderStory(block,story,copy,locale):null;
  const shown=[Boolean(told_),told(block,history,copy,locale,language,language===ORIGINAL?'W':'V'),descent(block,lemma,record,copy,locale,language),
    await parts(block,record,copy,locale,language),kin(block,record,copy,locale),attested(block,history,copy,locale),
    counted&&Boolean(inBook(block,history,copy,locale,language===ORIGINAL?'B':null))];
  if(shown.some(Boolean))parent.append(block);
  return {shown:shown.some(Boolean),story:told_};
}

/** How often the book says the word, part by part. */
function inBook(parent,history,copy,locale,tag=null){
  const book=history?.book;
  if(!book?.n)return null;
  const parts=Object.entries(book.parts??{}).map(([part,n])=>fill(copy.part,part,n));
  const line=el('p','cr-origin-book',`${fill(copy.inBook,times(book.n,locale))}${parts.length>1?` · ${parts.join(' · ')}`:''}`);
  if(tag)line.dataset.storySource=tag;
  parent.append(line);
  return line;
}

/** The pieces a story cites, numbered in the order it first cites them. */
export function storyOrder(sentences){
  const order=[];
  for(const sentence of sentences??[])for(const id of sentence.sources??[])if(!order.includes(id))order.push(id);
  return order;
}

/** The story in the reader's language (Russian for Russian, else English), each sentence followed by its pieces' numbers. */
function renderStory(parent,story,copy,locale){
  const language=copyLocale(locale)==='ru'?'ru':'en',sentences=story?.[language]??[];
  if(!sentences.length)return null;
  const order=storyOrder(sentences);
  const block=el('div','cr-story'),text=el('p','cr-story-text');text.lang=language;
  for(const [index,sentence] of sentences.entries()){
    if(index)text.append(document.createTextNode(' '));
    text.append(document.createTextNode(sentence.text));
    for(const id of sentence.sources??[]){
      const number=order.indexOf(id)+1,mark=el('button','cr-story-mark',String(number));
      mark.type='button';mark.dataset.target=id;mark.setAttribute('aria-label',fill(copy.storyMark,number));
      text.append(mark);
    }
  }
  const pieces=el('ol','cr-story-pieces');pieces.hidden=true;
  block.append(text,el('p','cr-story-note',copy.storyNote),pieces);
  parent.append(block);
  return {block,pieces,order};
}

const pieceLanguage=locale=>copyLocale(locale)==='ru'?'ru':'en';

const QUOTE_PAIRS={ru:['«','»'],de:['„','“'],en:['“','”']};
/** A sentence cut from a speech keeps the speech's opening or closing mark alone; the sheet quotes the sentence itself. */
export function paired(text,language){
  const [open,close]=QUOTE_PAIRS[language]??QUOTE_PAIRS.ru;
  let said=String(text??'').trim();
  const count=mark=>[...said].filter(letter=>letter===mark).length;
  if(said.startsWith(open)&&count(open)>count(close))said=said.slice(open.length).trimStart();
  if(said.endsWith(close)&&count(close)>count(open))said=said.slice(0,-close.length).trimEnd();
  return said;
}

/** A piece of the story the sheet shows nowhere else: a place of the book, the translators' words, a Wiktionary telling. */
export function renderPiece(item,id,piece,{copy,locale,history=null,onOpenPlace=null}){
  const language=pieceLanguage(locale);
  if(piece?.kind==='book-place'||piece?.kind==='key-place'){
    item.append(el('p','cr-story-piece-title',piece.title?.[language]??piece.title?.de??piece.chapter));
    const saidLanguage=piece.text?.[language]?language:ORIGINAL;
    const said=el('q','cr-origin-said',paired(piece.text?.[saidLanguage]??'',saidLanguage));said.lang=saidLanguage;
    item.append(said);
    if(piece.text?.de&&language!==ORIGINAL){const original=el('p','cr-story-original',paired(piece.text.de,ORIGINAL));original.lang=ORIGINAL;item.append(original);}
    if(piece.why?.[language])item.append(el('p','cr-origin-source',`${copy.treeReads}: ${piece.why[language]}`));
    if(piece.unit&&typeof onOpenPlace==='function'){
      const open=el('button','cr-origin-open',copy.open);open.type='button';
      open.addEventListener('click',()=>onOpenPlace({chapter:piece.chapter,unitId:piece.unit,start:piece.start,end:piece.end,language:ORIGINAL}));
      item.append(open);
    }
    return true;
  }
  if(piece?.kind==='translations'){
    const words=list=>(list??[]).map(([word,n])=>`${word} (${n})`).join(', ');
    const lines=[[copy.antonovsky,piece.words?.ru,piece.first?.ru],[copy.common,piece.words?.en,piece.first?.en]]
      .filter(([,list,first])=>list?.length||first)
      .map(([who,list,first])=>`${who}: ${words(list)||first}${first&&list?.length&&list[0][0]!==first?` · ${copy.firstPlace}: ${first}`:''}`);
    const tree=[piece.tree?.ru,piece.tree?.en].filter(Boolean).join(' / ');
    if(tree)lines.push(`${copy.tree}: ${tree}`);
    for(const line of lines)item.append(el('p','cr-origin-line',line));
    return lines.length>0;
  }
  if(piece?.kind==='wiktionary'){
    const [tag,edition]=id.split('.');
    telling(item,edition??piece.edition,piece,copy,tag==='W'?ORIGINAL:'ru');
    return true;
  }
  if(piece?.kind==='descent'&&piece.text?.[language]){item.append(el('p','cr-origin-line',piece.text[language]));return true;}
  if(id==='B'&&history)return Boolean(inBook(item,history,copy,locale));
  return false;
}

/** Bind a story's numbers to the pieces they cite: the sheet's own elements, else the pieces drawn under the story. */
function linkStory(body,shown,story,{copy,locale,history,onOpenPlace}){
  if(!shown)return;
  const view=body.ownerDocument?.defaultView??globalThis;
  const target=id=>body.querySelector(`[data-story-source="${globalThis.CSS?.escape?.(id)??id}"]`);
  for(const [index,id] of shown.order.entries()){
    if(target(id))continue;
    const item=el('li','cr-story-piece');item.dataset.storySource=id;item.value=index+1;
    if(renderPiece(item,id,story.pieces?.[id],{copy,locale,history,onOpenPlace})){shown.pieces.append(item);shown.pieces.hidden=false;}
  }
  if(!shown.pieces.hidden)shown.pieces.before(el('p','cr-origin-subhead',copy.storyPieces));
  shown.block.addEventListener('click',event=>{
    const mark=event.target?.closest?.('.cr-story-mark');if(!mark)return;
    const found=target(mark.dataset.target);if(!found)return;
    for(let at=found.parentElement;at&&at!==body;at=at.parentElement)if(at.tagName==='DETAILS')at.open=true;
    found.scrollIntoView?.({block:'center',behavior:'smooth'});
    found.classList.add('cr-story-lit');
    view.setTimeout(()=>found.classList.remove('cr-story-lit'),2400);
  });
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
  let shown=false,told_=null,story=null,storyHistory=null;
  // A translation's word comes from the word of the original it renders: that word's history comes first.
  const source=lang!==ORIGINAL?record?.renders?.[0]?.[0]:null;
  if(source){
    const heading=`${copy.renders} · ${source}`;
    const [sourceRecord,sourceHistory,sourceStory]=await Promise.all([originRecord(ORIGINAL,source).catch(()=>null),
      historyRecord(ORIGINAL,source).catch(()=>null),storyRecord(source).catch(()=>null)]);
    // The original's word comes with its whole history, how often the book says it included.
    const result=await wordOrigin(body,source,sourceRecord,copy,locale,ORIGINAL,heading,sourceHistory,sourceStory,true);
    if(!result.shown)body.append(el('h3','cr-origin-heading',heading));
    shown=true;told_=result.story;story=sourceStory;storyHistory=sourceHistory;
  }
  const own=lang===ORIGINAL?await storyRecord(lemma).catch(()=>null):null;
  const result=await wordOrigin(body,lemma,record,copy,locale,lang,source?lemma:null,history,own);
  shown=result.shown||shown;
  if(own){told_=result.story;story=own;storyHistory=history;}
  status.remove();
  if(!shown)body.append(el('p','cr-philology-muted',copy.unknown));
  inBook(body,history,copy,locale,lang===ORIGINAL?'B':null);
  if(record?.first){
    const [chapter,unitId,start,end]=record.first,row=el('p','cr-origin-first');
    row.append(document.createTextNode(`${copy.first}: ${chapterTitle(chapter,lang)} `));
    if(typeof onOpenPlace==='function'){const open=el('button','cr-origin-open',copy.open);open.type='button';open.addEventListener('click',()=>onOpenPlace({chapter,unitId,start,end,language:lang}));row.append(open);}
    body.append(row);
  }
  linkStory(body,told_,story,{copy,locale,history:storyHistory,onOpenPlace});
}

/** The parts of a word's history, each into `parent`, as the origin sheet draws them. */
export const renderTold=(parent,history,{locale='ru',language=ORIGINAL}={})=>told(parent,history,words(locale),locale,language);
export const renderAttested=(parent,history,{locale='ru'}={})=>attested(parent,history,words(locale),locale);
export const renderInBook=(parent,history,{locale='ru'}={})=>inBook(parent,history,words(locale),locale);
export const renderStoryText=(parent,story,{locale='ru'}={})=>renderStory(parent,story,words(locale),locale);
export const storyPiece=(item,id,piece,{locale='ru',history=null,onOpenPlace=null}={})=>renderPiece(item,id,piece,{copy:words(locale),locale,history,onOpenPlace});
