// Fragments instead of retellings.
//
// An authored form of a stand is {ru, en, cite}: its own words (a reading, a
// question, a connection — what a quotation cannot give) and the places of the
// work it rests on. A place is shown as the work's own text: the cited part of
// the original, and the same place in the reader's language, found through the
// alignment of the witnesses — never chosen by hand, never retold. The whole
// work stays one step away: the place opens in the parallel reading of its
// chapter, original beside translation, and from there in the corpus reader.

import {benchListens,layOnBench} from './bench/bridge.mjs';
import {chapterTitle} from './chapter-title.mjs';
import {readChapter,fragmentOf,displayText} from '../../../../bonsai/lib/witnesses.mjs';
import {readCorpusFragmentRange} from './corpus-fragment-range.mjs';
import {modernRussian,isPreReform,respell} from './orthography.mjs';
import {createLatestFragmentRequest} from './fragment-request.mjs';
import {copyLocale,localeText,localeValue,resolveLocale,shownLanguages} from './locale.mjs';

const ZARATHUSTRA='also-sprach-zarathustra';
const chapterFiles=import.meta.glob('../../../../bonsai/zarathustra/witnesses/*.json',{import:'default'});
// Vite serves JSON imports as JavaScript modules in development and bundles them.
// Do not request native JSON MIME semantics for these transformed modules.
const loadManifest=()=>import('../../../../bonsai/zarathustra/witnesses.json').then(module=>module.default);
// Each stand brings its own places of other works and its own alignment of the corpus works it cites.
const PLACES={
  zarathustra:()=>import('../../../../bonsai/zarathustra/fragments.json').then(module=>module.default),
  antiquity:()=>import('../../../../bonsai/antiquity/fragments.json').then(module=>module.default),
};
const ALIGNMENTS={
  zarathustra:()=>import('../../../../bonsai/zarathustra/alignment.json').then(module=>module.default),
  antiquity:()=>import('../../../../bonsai/antiquity/alignment.json').then(module=>module.default),
};
const CHAPTER_KEYS=Object.keys(chapterFiles).map(path=>path.split('/').pop().replace(/\.json$/,'')).sort((a,b)=>{const [pa,ca]=a.split('.'),[pb,cb]=b.split('.'),order=['I','II','III','IV'];return order.indexOf(pa)-order.indexOf(pb)||Number(ca)-Number(cb);});

const el=(tag,cls='',value)=>{const node=document.createElement(tag);if(cls)node.className=cls;if(value!==undefined)node.textContent=value;return node;};
const button=(value,cls='')=>{const node=el('button',cls,value);node.type='button';return node;};

/** The citations a form carries. */
export const citesOf=value=>Array.isArray(value?.cite)?value.cite.filter(cite=>cite&&typeof cite==='object'):[];
/** Whether a form says or shows anything. */
export const hasAuthored=(value,locale)=>citesOf(value).length>0||!!localeValue(value,locale).trim();

/**
 * Which witnesses a reader sees. Antiquity requests the interface language
 * exactly and offers the original separately; the other stand retains its
 * existing fallback when no witness translates into the interface language.
 */
export function readingLanguages(locale,original='de',available=['de','en','ru'],{exact=false}={}){
  if(exact){
    const language=resolveLocale(locale),hasOriginal=available.includes(original);
    if(language===original)return {primary:hasOriginal?[original]:[],alternate:null};
    return {primary:available.includes(language)?[language]:[],alternate:hasOriginal?original:null};
  }
  const [,translation]=shownLanguages(locale,original,available);
  if(!translation)return {primary:[original],alternate:null};
  if(translation===resolveLocale(locale))return {primary:[translation],alternate:original};
  return {primary:[original,translation],alternate:null};
}

// Chapter titles as the witnesses print them, without numbering or a closing period.
export {chapterTitle};
/**
 * A chapter's name for a reader: its title in the witness of the reader's language; where that witness omits the
 * chapter, the stand's own name for it; in a language without a witness, the English title as the catalog renders it.
 */
export function chapterName(chapter,locale){
  if(!chapter)return '';
  const language=resolveLocale(locale);
  if(language==='ru'||language==='de'||language==='en')return chapterTitle(chapter,language)||chapter.names?.[language]||chapterTitle(chapter,'en');
  const raw=chapter.titles?.en??'',translated=localeText(raw,language);
  return translated!==raw?translated.replace(/^[IVXLC]+\.\s*/,'').replace(/\.$/,'').trim():chapterTitle(chapter,'en');
}

/** Text of a fragment in one witness: the cited part of each unit, verse keeping its lines. */
// What a reader sees of a text in a language: Russian printed before 1918 in today's spelling. The place is cut
// from the source first, so citation offsets keep counting the source's own characters.
const shownIn=(language,text)=>language==='ru'?modernRussian(text):text;
function fragmentNodes(fragment,language,chapterKey=null){
  const nodes=[];
  for(const unit of fragment.units){
    const slice=unit.text.slice(unit.from,unit.to),source=slice.trim();
    const map=language==='ru'?respell(source):null;
    const text=map?.text??source;
    if(!text)continue;
    const node=el('p',unit.kind==='v'?'ps-fragment-line is-verse':'ps-fragment-line',text);
    // The witness unit a line shows, so that a selected word finds its reading in the word layer.
    if(chapterKey&&unit.id){node.dataset.chapter=chapterKey;node.dataset.unit=unit.id;node.dataset.offset=String(unit.from+slice.length-slice.trimStart().length);node.witnessOffsets=map?.toSource;}
    nodes.push(node);
  }
  return nodes;
}

const LIMIT=720;
/** A long place shows its beginning; the whole of it is one step away in the parallel reading. */
function clip(nodes){
  let used=0;const kept=[];
  for(const node of nodes){
    const text=node.textContent;
    if(used+text.length<=LIMIT){kept.push(node);used+=text.length;continue;}
    const room=LIMIT-used;
    if(room>120||!kept.length){const cut=text.slice(0,Math.max(room,120)),at=cut.lastIndexOf(' ');node.textContent=`${(at>60?cut.slice(0,at):cut).trimEnd()} …`;kept.push(node);}
    else kept.at(-1).textContent=`${kept.at(-1).textContent.trimEnd()} …`;
    return {nodes:kept,clipped:true};
  }
  return {nodes:kept,clipped:false};
}

/**
 * Fragments of one stand. `t` gives the stand's words, `openAddress` opens a
 * place in the corpus reader, `hint` marks an element with a hint.
 */
export function createFragments({profile='zarathustra',locale,t,provider=null,openAddress=null,hint=null,onError=null,host=()=>document.body,decorate=null}={}){
  const chapters=new Map();
  let manifest=null,places=null;
  const chapter=key=>{
    if(!chapters.has(key)){
      const path=Object.keys(chapterFiles).find(item=>item.endsWith(`/${key}.json`));
      chapters.set(key,path?chapterFiles[path]().then(readChapter):Promise.reject(new Error(`No chapter ${key}`)));
      chapters.get(key).catch(()=>chapters.delete(key));
    }
    return chapters.get(key);
  };
  const witnesses=()=>manifest??=loadManifest().catch(error=>{manifest=null;throw error;});
  const externalPlaces=()=>places??=(PLACES[profile]?.()??Promise.resolve({works:{},places:{}})).catch(error=>{places=null;throw error;});
  let alignment=null;
  const corpusAlignment=()=>alignment??=(ALIGNMENTS[profile]?.()??Promise.resolve({works:{},units:{}})).catch(error=>{alignment=null;throw error;});
  // Agent reading uses the same witness and raw citation ranges as the cards.
  // Bonsai offsets count UTF-16 positions in its saved raw witness; native
  // reader selectors count Unicode code points. Convert only at this boundary.
  async function sourceSelection(cite,{signal}={}){
    if(!provider||!cite||!Array.isArray(cite.u)||!cite.u.length)throw new TypeError('An exact TextUnit citation is required.');
    const first=cite.u[0],last=cite.u.at(-1);
    let documentId,versionId,revision,units,legacyOffsets=false;
    if(cite.w===ZARATHUSTRA&&cite.c){
      const [data,info]=await Promise.all([chapter(cite.c),witnesses()]);
      const a=data.index.de.get(first),b=data.index.de.get(last);
      if(a===undefined||b===undefined||b<a)throw new TypeError('The citation does not resolve in its named chapter.');
      ({documentId}=info.corpus);({versionId,revision}=info.corpus.dta);
      units=data.units.de.slice(a,b+1).map(unit=>({id:unit.id,text:unit.raw}));legacyOffsets=true;
    }else{
      const work=(await corpusAlignment()).works?.[cite.w];
      if(!work?.original)throw new TypeError('No exact source binding exists for this work.');
      ({documentId}=work);({versionId,revision}=work.original);
      units=await readCorpusFragmentRange(provider,work,work.original,first,last,{signal});
    }
    if((cite.versionId&&cite.versionId!==versionId)||(cite.revision&&cite.revision!==revision)
      ||(cite.documentId&&cite.documentId!==documentId))throw new TypeError('The citation names a different witness or text layer.');
    const nativeOffset=(text,offset)=>{
      if(!Number.isSafeInteger(offset)||offset<0||offset>(legacyOffsets?text.length:Array.from(text).length))throw new TypeError('Citation offset is outside its source unit.');
      if(!legacyOffsets)return offset;
      if(offset>0&&offset<text.length&&/[\uD800-\uDBFF]/u.test(text[offset-1])&&/[\uDC00-\uDFFF]/u.test(text[offset]))throw new TypeError('Citation offset splits a Unicode character.');
      return Array.from(text.slice(0,offset)).length;
    };
    const selections=units.map((unit,i)=>{
      const start=i===0&&cite.s!==undefined?nativeOffset(unit.text,cite.s):0;
      const end=i===units.length-1&&cite.e!==undefined?nativeOffset(unit.text,cite.e):Array.from(unit.text).length;
      if(end<start)throw new TypeError('Citation range is reversed.');
      return {unitId:unit.id,start,end,raw:unit.text};
    });
    const doc=await provider.document({documentId,versionId,revision,signal});
    const version=doc.versions?.find(value=>value.id===versionId&&value.revision===revision);
    if(!version?.source)throw new TypeError('The exact witness version is unavailable.');
    return {documentId,versionId,revision,source:version.source,selections};
  }
  const words=key=>t(key);
  // A quotation is taken away with its source: the words shown, in each language shown, and the line naming where they stand.
  function copyTool(figure,tools){
    const clipboard=globalThis.navigator?.clipboard;if(!clipboard?.writeText)return;
    const copy=button(words('fragmentCopy'),'text-button ps-fragment-copy');
    copy.addEventListener('click',()=>{
      const [open,close]=String(locale()).startsWith('ru')?['«','»']:['“','”'],sources=[...figure.querySelectorAll('.ps-fragment-source')];
      const text=[...figure.querySelectorAll('.ps-fragment-text')].map((quote,index)=>`${open}${quote.innerText.trim()}${close}${sources[index]?`\n— ${sources[index].textContent.trim()}`:''}`).join('\n\n');
      void clipboard.writeText(text).then(()=>{copy.textContent=words('fragmentCopied');setTimeout(()=>{copy.textContent=words('fragmentCopy');},1600);}).catch(()=>{});
    });
    tools.append(copy);
  }
  // A quotation of the book goes onto the workbench, read in the witness the reader sees, where the stand has a bench.
  function benchTool(tools,cite,language){
    if(!benchListens())return;
    const lay=button(words('fragmentBench'),'text-button ps-fragment-bench');
    lay.addEventListener('click',()=>void layOnBench({cite,language}).then(laid=>{
      lay.textContent=words(laid?'fragmentBenched':'unavailable');setTimeout(()=>{lay.textContent=words('fragmentBench');},1600);}));
    tools.append(lay);
  }
  const local=value=>localeValue(value,locale());

  // ── The parallel reading: a chapter, original beside translation, row by row. ──
  let reader=null;
  const chapterRequests=createLatestFragmentRequest();
  function parallel(){
    if(reader)return reader;
    const dialog=el('dialog','ps-dialog ps-parallel');
    const head=el('div','dialog-top ps-parallel-top'),titles=el('div','ps-parallel-titles'),nav=el('div','ps-parallel-nav');
    const previous=button('←','icon ps-parallel-step'),next=button('→','icon ps-parallel-step'),close=button('×','icon');
    nav.append(previous,next,close);head.append(titles,nav);
    const body=el('div','ps-parallel-body'),foot=el('div','ps-parallel-foot');
    dialog.append(head,body,foot);
    close.addEventListener('click',()=>dialog.close());
    dialog.addEventListener('close',()=>chapterRequests.cancel());
    (host()??document.body).append(dialog);
    reader={dialog,titles,body,foot,previous,next,close,current:null};
    previous.addEventListener('click',()=>{const at=CHAPTER_KEYS.indexOf(reader.current?.key);if(at>0)void showChapter(CHAPTER_KEYS[at-1]).catch(error=>onError?.(error));});
    next.addEventListener('click',()=>{const at=CHAPTER_KEYS.indexOf(reader.current?.key);if(at>=0&&at<CHAPTER_KEYS.length-1)void showChapter(CHAPTER_KEYS[at+1]).catch(error=>onError?.(error));});
    return reader;
  }
  async function showChapter(key,cite=null){
    const view=parallel(),current=chapterRequests.begin(),[data,info]=await Promise.all([chapter(key),witnesses()]);
    if(!current()||reader!==view)return;
    view.current={key,cite};
    const {primary,alternate}=readingLanguages(locale());
    const columns=[...new Set(['de',...primary.filter(language=>language!=='de'),...(alternate&&alternate!=='de'?[alternate]:[])])].filter(language=>info.witnesses[language]);
    const shownColumns=columns.length>1?columns.slice(0,2):columns;
    view.titles.replaceChildren(...shownColumns.map(language=>{const h=el(language==='de'?'h2':'p',language==='de'?'':'ps-parallel-subtitle',chapterTitle(data,language)||'—');h.lang=language==='zh-Hans'?'zh':language;return h;}));
    const at=CHAPTER_KEYS.indexOf(key);view.previous.disabled=at<=0;view.next.disabled=at<0||at>=CHAPTER_KEYS.length-1;
    view.previous.setAttribute('aria-label',words('parallelPrevious'));view.next.setAttribute('aria-label',words('parallelNext'));view.close.setAttribute('aria-label',words('close'));
    hint?.(view.previous,words('parallelPrevious'));hint?.(view.next,words('parallelNext'));
    const cited=new Map();
    if(cite){
      for(const language of shownColumns){
        const fragment=fragmentOf(cite,data,language);
        for(const unit of fragment?.units??[])cited.set(`${language}:${unit.index}`,[unit.from,unit.to]);
      }
    }
    const grid=el('div','ps-parallel-grid');grid.dataset.columns=String(shownColumns.length);
    let first=null;
    data.rows.forEach(row=>{
      const line=el('div','ps-parallel-row');
      for(const language of shownColumns){
        const cell=el('div','ps-parallel-cell');cell.lang=language;
        const indexes=row[language]??[];
        if(!indexes.length){cell.classList.add('is-absent');if(language!=='de')hint?.(cell,words('fragmentOmitted'));}
        for(const index of indexes){
          const unit=data.units[language][index],shown=displayText(unit.raw,unit.kind),p=el('p',unit.kind==='v'?'is-verse':'');p.dataset.chapter=key;p.dataset.unit=unit.id;p.dataset.offset='0';
          const span=cited.get(`${language}:${index}`),map=language==='ru'?respell(shown.text):null,text=map?.text??shown.text;
          p.witnessOffsets=map?.toSource;
          if(span&&span[1]>span[0]){
            const start=map?.toDisplay[span[0]]??span[0],end=map?.toDisplay[span[1]]??span[1];
            p.append(text.slice(0,start),el('mark','',text.slice(start,end)),text.slice(end));line.classList.add('is-cited');first??=line;
          }else p.textContent=text;
          cell.append(p);
        }
        line.append(cell);
      }
      grid.append(line);
    });
    view.body.replaceChildren(grid);
    // The book's own words open on their origin, as they do in the cards.
    decorate?.(grid);
    // The editions themselves, in the reader of the corpus, where a place of this chapter stands there too.
    view.foot.replaceChildren();
    const firstRow=first?data.rows[[...grid.children].indexOf(first)]:data.rows[0];
    // Only the editions of the columns shown: the reader is never offered a translation into another language.
    const editions=[];
    for(const language of shownColumns){
      const unit=data.units[language]?.[firstRow?.[language]?.[0]],version=language==='de'?info.corpus.dta:info.corpus[language];
      if(openAddress&&unit&&version)editions.push([language==='zh-Hans'?'ZH':language.toUpperCase(),{documentId:info.corpus.documentId,versionId:version.versionId,revision:version.revision,unitId:unit.id}]);
    }
    // One action, one button per edition: the footer stays one line even on a phone.
    if(editions.length){
      const group=el('span','ps-parallel-editions');group.append(el('span','ps-parallel-editions-label',`↗ ${words('fragmentEdition')}`));
      for(const [code,address] of editions){const open=button(code,'text-button');open.setAttribute('aria-label',`${words('fragmentEdition')} · ${code}`);open.addEventListener('click',()=>{view.dialog.close();void openAddress(address).catch(error=>onError?.(error));});group.append(open);}
      view.foot.append(group);
    }
    // The credits take one line and open in full on demand.
    const credit=el('p','ps-parallel-credit',shownColumns.map(language=>info.witnesses[language]?.attribution).filter(Boolean).join(' · '));
    credit.tabIndex=0;credit.setAttribute('role','button');credit.setAttribute('aria-expanded','false');
    const toggleCredit=()=>credit.setAttribute('aria-expanded',String(credit.classList.toggle('is-open')));
    credit.addEventListener('click',toggleCredit);credit.addEventListener('keydown',event=>{if(event.key==='Enter'||event.key===' '){event.preventDefault();toggleCredit();}});
    view.foot.append(credit);
    if(!view.dialog.open)view.dialog.showModal();
    (first??grid.firstElementChild)?.scrollIntoView({block:first?'center':'start'});
  }

  // ── One fragment: the place, its caption, the way to the original and to the whole. ──
  function captionFor(chapterData,language,info){
    const witness=info.witnesses[language],short=witness?.short?.[resolveLocale(locale())]??witness?.short?.en??'';
    const part=chapterData.key?.split('.')[0],year=language==='de'?witness?.years?.[part]:null;
    return [chapterTitle(chapterData,language),[short,year].filter(Boolean).join(' ')].filter(Boolean).join(' · ');
  }
  async function fillZarathustra(figure,cite){
    const [data,info]=await Promise.all([chapter(cite.c),witnesses()]);
    const {primary,alternate}=readingLanguages(locale());
    let showing=[...primary];
    const omitted=showing.filter(language=>language!=='de'&&fragmentOf(cite,data,language)?.omitted);
    if(omitted.length)showing=showing.filter(language=>!omitted.includes(language)).concat(showing.includes('de')?[]:['de']);
    const paint=()=>{
      figure.replaceChildren();
      for(const language of showing){
        const fragment=fragmentOf(cite,data,language);if(!fragment)continue;
        const quote=el('blockquote','ps-fragment-text');quote.lang=language;
        const {nodes}=clip(fragmentNodes(fragment,language,cite.c));quote.append(...nodes);
        const caption=el('figcaption','ps-fragment-cite');caption.append(el('span','ps-fragment-source',captionFor(data,language,info)));
        figure.append(quote,caption);
      }
      const tools=el('div','ps-fragment-tools');
      const other=alternate&&!omitted.length?(showing.includes(alternate)?primary:[alternate]):null;
      if(other){
        const swap=button(showing.includes(alternate)?words('fragmentTranslation'):words('fragmentOriginal'),'text-button ps-fragment-swap');
        swap.setAttribute('aria-pressed',String(showing.includes(alternate)));
        swap.addEventListener('click',()=>{showing=other;paint();figure.querySelector('.ps-fragment-swap')?.focus({preventScroll:true});});
        tools.append(swap);
      }
      if(omitted.length){const absent=el('span','ps-fragment-absent','∅');absent.setAttribute('aria-label',words('fragmentOmitted'));hint?.(absent,words('fragmentOmitted'));tools.append(absent);}
      const whole=button(`↗ ${words('fragmentWhole')}`,'text-button ps-fragment-whole');
      whole.addEventListener('click',()=>void showChapter(cite.c,cite).catch(error=>onError?.(error)));
      tools.append(whole);benchTool(tools,cite,showing.find(language=>language!=='de')??'de');copyTool(figure,tools);figure.append(tools);painted(figure);
    };
    paint();
  }
  async function fillPlace(figure,cite){
    const data=await externalPlaces(),work=data.works?.[cite.w],place=data.places?.[cite.w]?.[cite.loc];
    if(!work||!place)throw new Error(`No place ${cite.w} ${cite.loc}`);
    // A place may give a language from another witness of it (the translation of a book the usual one lacks).
    const witnessOf=language=>work.witnesses?.[place.witness?.[language]??language];
    const available=Object.keys(place.texts??{}).filter(language=>place.texts[language]);
    const {primary,alternate}=readingLanguages(locale(),work.original,available,{exact:profile==='antiquity'});
    let showing=primary.filter(language=>available.includes(language));
    if(!showing.length&&profile!=='antiquity')showing=[work.original].filter(language=>available.includes(language));
    const paint=()=>{
      figure.replaceChildren();
      // A protected work the stand may not quote is shown by its locator alone.
      if(!showing.length){
        if(profile==='antiquity')figure.append(el('p','ps-fragment-wait',words('fragmentTranslationUnavailable')));
        const caption=el('figcaption','ps-fragment-cite');caption.append(el('span','ps-fragment-source',[local(work.author),local(work.title),local(place.label)].filter(Boolean).join(' · ')));figure.append(caption);
      }
      for(const language of showing){
        const text=place.texts[language];
        const narrowed=shownIn(language,language===work.original&&Number.isInteger(cite.s)?text.slice(cite.s,Number.isInteger(cite.e)?cite.e:text.length):text);
        const quote=el('blockquote','ps-fragment-text');quote.lang=language==='zh-Hans'?'zh':language;quote.dataset.termWorks=cite.w;
        const {nodes}=clip(narrowed.split(/\n{2,}/).map(part=>el('p','ps-fragment-line',part.trim())).filter(node=>node.textContent));
        quote.append(...nodes);
        const witness=witnessOf(language);
        const caption=el('figcaption','ps-fragment-cite');
        caption.append(el('span','ps-fragment-source',[local(work.author),local(work.title),local(place.label),local(witness?.short),language==='ru'&&isPreReform(text)?words('modernSpelling'):''].filter(Boolean).join(' · ')));
        figure.append(quote,caption);
      }
      const tools=el('div','ps-fragment-tools');
      const other=alternate&&available.includes(alternate)?(showing.includes(alternate)?primary.filter(language=>available.includes(language)):[alternate]):null;
      if(other&&(other.length||profile==='antiquity'&&showing.includes(alternate))&&other.join()!==showing.join()){
        const swap=button(showing.includes(alternate)?words('fragmentTranslation'):words('fragmentOriginal'),'text-button ps-fragment-swap');
        swap.setAttribute('aria-pressed',String(showing.includes(alternate)));
        swap.addEventListener('click',()=>{showing=other;paint();figure.querySelector('.ps-fragment-swap')?.focus({preventScroll:true});});
        tools.append(swap);
      }
      // The source of the text shown: the place's own page in that witness, the place's page, or the witness.
      // A link says what it opens: the whole work, the page a quotation is taken from, or where the book can be found.
      const witness=witnessOf(showing[0]);
      const [url,opens]=place.links?.[showing[0]]?[place.links[showing[0]],'work']:place.url?[place.url,place.opens??'book']:[witness?.url,witness?.opens??'work'];
      if(typeof url==='string'&&/^https:\/\//.test(url)){const link=el('a','text-button ps-fragment-whole',`↗ ${words({quotation:'fragmentQuoted',book:'fragmentBook'}[opens]??'fragmentWhole')}`);link.href=url;link.target='_blank';link.rel='noopener noreferrer';tools.append(link);}
      copyTool(figure,tools);figure.append(tools);painted(figure);
    };
    paint();
  }
  // ── A place of a corpus work: the original's units from the corpus, the translation through the stand's alignment. ──
  const unitsFrom=(work,version,first,last=first)=>readCorpusFragmentRange(provider,work,version,first,last);
  const overlaps=(a,b,s,e)=>a<e&&b>s;
  // The corpus names a place in Russian ("Книга 6 · 509", "Глава 52", "Строки от 146"); a caption names it in
  // the reader's language. An edition's chunk number ("Unit 59") names no place and is left out.
  const PLACE_WORDS=[['Книга','refBook'],['Глава','refChapter'],['Раздел','refSection'],['Фрагмент','refFragment'],['Строки от','refLines']];
  const placeName=raw=>{
    if(!raw||/^Unit \d+$/.test(raw))return '';
    // A collected edition names a dialogue by its slug beside its title ("crito · Критон"); the reader sees the title.
    const parts=raw.split(' · '),reference=parts.length>1?parts.filter(part=>!/^[a-z][a-z0-9-]*$/.test(part)).join(' · '):raw;
    if(resolveLocale(locale())==='ru')return reference;
    let value=reference.replace(/Предисловие Арриана/gu,words('refArrian'));
    for(const [word,key] of PLACE_WORDS)value=value.replace(new RegExp(`${word}\\s+([^\\s·]+)`,'gu'),(_,n)=>words(key).replace('{n}',n));
    return value;
  };
  async function fillCorpus(figure,cite){
    if(!provider)throw new Error('No corpus provider');
    const data=await corpusAlignment(),work=data.works?.[cite.w];
    if(!work)throw new Error(`No alignment for ${cite.w}`);
    const [first,last]=cite.u;
    const originals=await unitsFrom(work,work.original,first,last);
    if(!originals.length)throw new Error(`No units for ${cite.w}`);
    const span=(unit,index)=>{const length=Array.from(unit.text).length;return [index===0&&Number.isInteger(cite.s)?cite.s:0,index===originals.length-1&&Number.isInteger(cite.e)?cite.e:length];};
    // A translation shows the place only where its sentence pairs meet the cited span (an entry aligned by
    // units alone gives its whole counterpart units); otherwise the reader gets the languages that do.
    const meets=(unit,index,entry)=>{const [a,b]=span(unit,index);return (entry.pairs??[]).some(([source])=>source.some(([s,e])=>overlaps(s,e,a,b)));};
    const covers=item=>originals.some((unit,index)=>{const entry=data.units?.[unit.id]?.t?.[item.versionId];return !!entry&&(!(entry.pairs??[]).length||meets(unit,index,entry));});
    const available=[work.original.language,...work.translations.filter(covers).map(item=>item.language)];
    const {primary,alternate}=readingLanguages(locale(),work.original.language,available,{exact:profile==='antiquity'});
    const versionFor=language=>language===work.original.language?work.original:work.translations.find(item=>item.language===language&&covers(item));
    let showing=primary.filter(language=>versionFor(language));
    if(!showing.length&&profile!=='antiquity')showing=[work.original.language];
    const texts=new Map();
    async function textOf(language){
      if(texts.has(language))return texts.get(language);
      const version=versionFor(language);
      if(language===work.original.language){const value=originals.map((unit,index)=>{const [a,b]=span(unit,index);return {id:unit.id,reference:unit.reference,text:Array.from(unit.text).slice(a,b).join('').trim()};});texts.set(language,value);return value;}
      // The translation's sentences joined with the cited ones, unit by unit of the translation.
      const wanted=new Map();
      originals.forEach((unit,index)=>{
        const entry=data.units?.[unit.id]?.t?.[version.versionId];if(!entry)return;
        const [a,b]=span(unit,index);
        let found=false;
        for(const [source,target] of entry.pairs??[]){
          if(!source.some(([s,e])=>overlaps(s,e,a,b)))continue;found=true;
          for(const [i,s,e] of target){const id=entry.u[i],old=wanted.get(id);wanted.set(id,old?[Math.min(old[0],s),Math.max(old[1],e)]:[s,e]);}
        }
        if(!found&&!(entry.pairs??[]).length)for(const id of entry.u)if(!wanted.has(id))wanted.set(id,null);
      });
      const value=[];
      for(const [id,range] of wanted){
        const [unit]=await unitsFrom(work,version,id);if(!unit)continue;
        const chars=Array.from(unit.text),source=(range?chars.slice(range[0],range[1]):chars).join('').trim();value.push({id,reference:unit.reference,text:shownIn(language,source),respelled:language==='ru'&&isPreReform(source)});
      }
      texts.set(language,value);return value;
    }
    const paints=createLatestFragmentRequest();
    const paint=async()=>{
      const current=paints.begin();
      const parts=await Promise.all(showing.map(async language=>[language,await textOf(language)]));
      if(!current())return;
      figure.replaceChildren();
      if(!showing.length&&profile==='antiquity')figure.append(el('p','ps-fragment-wait',words('fragmentTranslationUnavailable')));
      for(const [language,value] of parts){
        if(!value.length)continue;
        const version=versionFor(language);
        const quote=el('blockquote','ps-fragment-text');quote.lang=language==='grc'?'grc':language==='la'?'la':language;quote.dataset.edition=version.versionId;quote.dataset.termWorks=cite.w;
        const {nodes}=clip(value.flatMap(item=>item.text.split(/\n{2,}/)).map(part=>el('p','ps-fragment-line',part.replace(/\s*\n\s*/g,' ').trim())).filter(node=>node.textContent));
        quote.append(...nodes);
        const references=[...new Set(value.map(item=>placeName(item.reference)).filter(Boolean))];
        const caption=el('figcaption','ps-fragment-cite');
        caption.append(el('span','ps-fragment-source',[...saidOnce(String(version?.label??'').split(' · ')),references.length?references[0]+(references.length>1?`–${references.at(-1)}`:''):'',value.some(item=>item.respelled)?words('modernSpelling'):''].filter(Boolean).join(' · ')));
        figure.append(quote,caption);
      }
      const tools=el('div','ps-fragment-tools');
      if(alternate&&versionFor(alternate)){
        const back=primary.filter(language=>versionFor(language));
        const other=showing.includes(alternate)?back:[alternate];
        if((other.length||profile==='antiquity'&&showing.includes(alternate))&&other.join()!==showing.join()){
          const swap=button(showing.includes(alternate)?words('fragmentTranslation'):words('fragmentOriginal'),'text-button ps-fragment-swap');
          swap.setAttribute('aria-pressed',String(showing.includes(alternate)));
          swap.addEventListener('click',()=>{showing=other;void paint().then(()=>figure.querySelector('.ps-fragment-swap')?.focus({preventScroll:true})).catch(error=>onError?.(error));});
          tools.append(swap);
        }
      }
      if(openAddress){
        const shown=versionFor(showing[0])??work.original,unitId=shown===work.original?first:(texts.get(showing[0])??[])[0]?.id;
        if(unitId){const whole=button(`↗ ${words('fragmentWhole')}`,'text-button ps-fragment-whole');whole.addEventListener('click',()=>void openAddress({documentId:work.documentId,versionId:shown.versionId,revision:shown.revision,unitId}).catch(error=>onError?.(error)));tools.append(whole);}
      }
      copyTool(figure,tools);figure.append(tools);painted(figure);
    };
    await paint();
  }
  // A figure tells the page when its text is shown, and again when the reader turns it to another language.
  const painted=figure=>figure.dispatchEvent(new CustomEvent('ps-fragment-painted',{bubbles:true}));
  // A part of an edition's label that an earlier part already says is said once ("…, 1934 · 1934").
  const saidOnce=parts=>parts.filter((part,at)=>part&&!parts.slice(0,at).some(earlier=>earlier.includes(part)));
  // Quotations of one source in a row name it once, under the last of them.
  const sourceOf=figure=>[...figure.querySelectorAll(':scope > .ps-fragment-cite')].map(node=>node.textContent).join('\n');
  function shareSource(box){
    const figures=[...box.querySelectorAll(':scope > .ps-fragment')];
    figures.forEach((figure,at)=>{const own=sourceOf(figure),next=figures[at+1];figure.classList.toggle('is-same-source',!!own&&!!next&&own===sourceOf(next));});
  }
  // Places of one source beside the same words name it once, above the first of them.
  function shareGroundSource(aside){
    const figures=[...aside.querySelectorAll(':scope > .rb-ground > .ps-fragment')];
    figures.forEach((figure,at)=>{const own=sourceOf(figure),before=figures[at-1];figure.parentElement.classList.toggle('is-same-source',!!own&&!!before&&own===sourceOf(before));});
  }
  /** One cited place as a figure, filled when its text arrives. */
  function fragment(cite){
    const figure=el('figure','ps-fragment');figure.dataset.work=cite.w;figure.setAttribute('aria-busy','true');
    figure.append(el('p','ps-fragment-wait','…'));
    const fill=cite.w===ZARATHUSTRA&&cite.c?fillZarathustra(figure,cite):cite.loc?fillPlace(figure,cite):Array.isArray(cite.u)?fillCorpus(figure,cite):Promise.reject(new Error(`No witness for ${cite.w}`));
    fill.then(()=>figure.removeAttribute('aria-busy')).catch(error=>{figure.removeAttribute('aria-busy');figure.replaceChildren(el('p','ps-fragment-wait',words('fragmentUnavailable')));onError?.(error);});
    return figure;
  }
  /**
   * Append a form: the places it cites, then its own words. Returns the
   * element appended, or null when the form neither says nor shows anything.
   */
  function render(parent,value,{className='body',tag='p',grounds=false}={}){
    const cites=citesOf(value),own=local(value).trim();
    if(!cites.length&&!own)return null;
    const box=el('div','ps-authored');
    if(grounds&&(own||grounds==='places')){
      // In the book the stand's own words lead and the places they rest on stand beside them: each place shows
      // its first lines and its source, and opens whole where it stands. A list of places alone is laid out the same way.
      box.classList.add('rb-claim');if(own){const words=el('div','rb-claim-text');words.append(el(tag,className,own));box.append(words);}else box.classList.add('is-places');
      if(cites.length){const aside=el('div','rb-grounds');for(const cite of cites)aside.append(ground(cite));if(cites.length>1)aside.addEventListener('ps-fragment-painted',()=>shareGroundSource(aside));box.append(aside);}
    }else{
      for(const cite of cites)box.append(fragment(cite));
      if(cites.length>1)box.addEventListener('ps-fragment-painted',()=>shareSource(box));
      if(own)box.append(el(tag,className,own));
    }
    parent.append(box);
    return box;
  }
  /** A place beside the words it grounds: its first lines, opened whole on request. */
  function ground(cite){
    const wrap=el('div','rb-ground'),figure=fragment(cite),toggle=button(words('groundOpen'),'rb-ground-toggle');
    toggle.setAttribute('aria-expanded','false');
    toggle.addEventListener('click',()=>{const open=!wrap.classList.contains('is-open');wrap.classList.toggle('is-open',open);toggle.setAttribute('aria-expanded',String(open));toggle.textContent=words(open?'groundClose':'groundOpen');});
    // A folded place opens where it is pressed, as well as by its own button.
    figure.addEventListener('click',event=>{if(!wrap.classList.contains('is-open')&&!event.target.closest?.('button,a'))toggle.click();});
    wrap.append(figure,toggle);return wrap;
  }
  return {render,fragment,showChapter,sourceSelection,citesOf,hasAuthored:value=>hasAuthored(value,locale()),get reader(){return reader;},dispose(){chapterRequests.cancel();reader?.dialog.remove();reader=null;}};
}

export {CHAPTER_KEYS,copyLocale};
