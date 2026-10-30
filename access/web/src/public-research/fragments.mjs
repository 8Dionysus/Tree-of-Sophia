// Fragments instead of retellings.
//
// An authored form of a stand is {ru, en, cite}: its own words (a reading, a
// question, a connection — what a quotation cannot give) and the places of the
// work it rests on. A place is shown as the work's own text: the cited part of
// the original, and the same place in the reader's language, found through the
// alignment of the witnesses — never chosen by hand, never retold. The whole
// work stays one step away: the place opens in the parallel reading of its
// chapter, original beside translation, and from there in the corpus reader.

import {readChapter,fragmentOf,displayText} from '../../../../bonsai/lib/witnesses.mjs';
import {copyLocale,localeValue,resolveLocale} from './locale.mjs';

const ZARATHUSTRA='also-sprach-zarathustra';
const chapterFiles=import.meta.glob('../../../../bonsai/zarathustra/witnesses/*.json',{import:'default'});
const loadManifest=()=>import('../../../../bonsai/zarathustra/witnesses.json',{with:{type:'json'}}).then(module=>module.default);
// Each stand brings its own places of other works and its own alignment of the corpus works it cites.
const PLACES={
  zarathustra:()=>import('../../../../bonsai/zarathustra/fragments.json',{with:{type:'json'}}).then(module=>module.default),
  antiquity:()=>import('../../../../bonsai/antiquity/fragments.json',{with:{type:'json'}}).then(module=>module.default),
};
const ALIGNMENTS={
  zarathustra:()=>import('../../../../bonsai/zarathustra/alignment.json',{with:{type:'json'}}).then(module=>module.default),
  antiquity:()=>import('../../../../bonsai/antiquity/alignment.json',{with:{type:'json'}}).then(module=>module.default),
};
const CHAPTER_KEYS=Object.keys(chapterFiles).map(path=>path.split('/').pop().replace(/\.json$/,'')).sort((a,b)=>{const [pa,ca]=a.split('.'),[pb,cb]=b.split('.'),order=['I','II','III','IV'];return order.indexOf(pa)-order.indexOf(pb)||Number(ca)-Number(cb);});

const el=(tag,cls='',value)=>{const node=document.createElement(tag);if(cls)node.className=cls;if(value!==undefined)node.textContent=value;return node;};
const button=(value,cls='')=>{const node=el('button',cls,value);node.type='button';return node;};

/** The citations a form carries. */
export const citesOf=value=>Array.isArray(value?.cite)?value.cite.filter(cite=>cite&&typeof cite==='object'):[];
/** Whether a form says or shows anything. */
export const hasAuthored=(value,locale)=>citesOf(value).length>0||!!localeValue(value,locale).trim();

/**
 * Which witnesses a reader sees: the translation into the interface language
 * with the original one step away; the original alone in German; the original
 * beside the English for the languages no witness translates into.
 */
export function readingLanguages(locale,original='de',available=['de','en','ru']){
  const language=resolveLocale(locale);
  if(language===original)return {primary:[original],alternate:null};
  if(available.includes(language))return {primary:[language],alternate:original};
  return {primary:[original,...(available.includes('en')&&original!=='en'?['en']:[])],alternate:null};
}

// Chapter titles as the witnesses print them, without numbering or a closing period.
const SMALL=new Set(['a','an','and','as','at','by','for','from','in','of','on','or','the','to','with']);
export function chapterTitle(chapter,language){
  let value=(chapter?.titles?.[language]??'').replace(/\.$/,'').trim();
  if(language==='en'){
    value=value.replace(/^[IVXLC]+\.\s*/,'');
    if(value===value.toUpperCase())value=value.toLowerCase().split(' ').map((word,index)=>index&&SMALL.has(word)?word:word.replace(/\p{L}/u,letter=>letter.toUpperCase())).join(' ');
  }
  return value;
}

/** Text of a fragment in one witness: the cited part of each unit, verse keeping its lines. */
function fragmentNodes(fragment){
  const nodes=[];
  for(const unit of fragment.units){
    const text=unit.text.slice(unit.from,unit.to).trim();
    if(text)nodes.push(el('p',unit.kind==='v'?'ps-fragment-line is-verse':'ps-fragment-line',text));
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
export function createFragments({profile='zarathustra',locale,t,provider=null,openAddress=null,hint=null,onError=null,host=()=>document.body}={}){
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
  const words=key=>t(key);
  const local=value=>localeValue(value,locale());

  // ── The parallel reading: a chapter, original beside translation, row by row. ──
  let reader=null;
  function parallel(){
    if(reader)return reader;
    const dialog=el('dialog','ps-dialog ps-parallel');
    const head=el('div','dialog-top ps-parallel-top'),titles=el('div','ps-parallel-titles'),nav=el('div','ps-parallel-nav');
    const previous=button('←','icon ps-parallel-step'),next=button('→','icon ps-parallel-step'),close=button('×','icon');
    nav.append(previous,next,close);head.append(titles,nav);
    const body=el('div','ps-parallel-body'),foot=el('div','ps-parallel-foot');
    dialog.append(head,body,foot);
    close.addEventListener('click',()=>dialog.close());
    (host()??document.body).append(dialog);
    reader={dialog,titles,body,foot,previous,next,close,current:null};
    previous.addEventListener('click',()=>{const at=CHAPTER_KEYS.indexOf(reader.current?.key);if(at>0)void showChapter(CHAPTER_KEYS[at-1]);});
    next.addEventListener('click',()=>{const at=CHAPTER_KEYS.indexOf(reader.current?.key);if(at>=0&&at<CHAPTER_KEYS.length-1)void showChapter(CHAPTER_KEYS[at+1]);});
    return reader;
  }
  async function showChapter(key,cite=null){
    const view=parallel(),[data,info]=await Promise.all([chapter(key),witnesses()]);
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
          const unit=data.units[language][index],shown=displayText(unit.raw,unit.kind),p=el('p',unit.kind==='v'?'is-verse':'');
          const span=cited.get(`${language}:${index}`);
          if(span&&span[1]>span[0]){p.append(shown.text.slice(0,span[0]));const mark=el('mark','',shown.text.slice(span[0],span[1]));p.append(mark,shown.text.slice(span[1]));line.classList.add('is-cited');first??=line;}
          else p.textContent=shown.text;
          cell.append(p);
        }
        line.append(cell);
      }
      grid.append(line);
    });
    view.body.replaceChildren(grid);
    // The editions themselves, in the reader of the corpus, where a place of this chapter stands there too.
    view.foot.replaceChildren();
    const firstRow=first?data.rows[[...grid.children].indexOf(first)]:data.rows[0];
    const deUnit=data.units.de[firstRow?.de?.[0]??0],enUnit=data.units.en?.[firstRow?.en?.[0]??0];
    const editions=[];
    if(openAddress&&deUnit?.corpus?.[0])editions.push(['DE',{documentId:info.corpus.documentId,versionId:info.corpus.de.versionId,revision:info.corpus.de.revision,unitId:deUnit.corpus[0]}]);
    if(openAddress&&enUnit)editions.push(['EN',{documentId:info.corpus.documentId,versionId:info.corpus.en.versionId,revision:info.corpus.en.revision,unitId:enUnit.id}]);
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
        const {nodes}=clip(fragmentNodes(fragment));quote.append(...nodes);
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
      tools.append(whole);figure.append(tools);
    };
    paint();
  }
  async function fillPlace(figure,cite){
    const data=await externalPlaces(),work=data.works?.[cite.w],place=data.places?.[cite.w]?.[cite.loc];
    if(!work||!place)throw new Error(`No place ${cite.w} ${cite.loc}`);
    const available=Object.keys(place.texts??{}).filter(language=>place.texts[language]);
    const {primary,alternate}=readingLanguages(locale(),work.original,available);
    let showing=primary.filter(language=>available.includes(language));
    if(!showing.length)showing=[work.original].filter(language=>available.includes(language));
    const paint=()=>{
      figure.replaceChildren();
      // A protected work the stand may not quote is shown by its locator alone.
      if(!showing.length){const caption=el('figcaption','ps-fragment-cite');caption.append(el('span','ps-fragment-source',[local(work.author),local(work.title),local(place.label)].filter(Boolean).join(' · ')));figure.append(caption);}
      for(const language of showing){
        const text=place.texts[language];
        const narrowed=language===work.original&&Number.isInteger(cite.s)?text.slice(cite.s,Number.isInteger(cite.e)?cite.e:text.length):text;
        const quote=el('blockquote','ps-fragment-text');quote.lang=language==='zh-Hans'?'zh':language;
        const {nodes}=clip(narrowed.split(/\n{2,}/).map(part=>el('p','ps-fragment-line',part.trim())).filter(node=>node.textContent));
        quote.append(...nodes);
        const witness=work.witnesses?.[language];
        const caption=el('figcaption','ps-fragment-cite');
        caption.append(el('span','ps-fragment-source',[local(work.author),local(work.title),local(place.label),local(witness?.short)].filter(Boolean).join(' · ')));
        figure.append(quote,caption);
      }
      const tools=el('div','ps-fragment-tools');
      const other=alternate&&available.includes(alternate)?(showing.includes(alternate)?primary.filter(language=>available.includes(language)):[alternate]):null;
      if(other?.length&&other.join()!==showing.join()){
        const swap=button(showing.includes(alternate)?words('fragmentTranslation'):words('fragmentOriginal'),'text-button ps-fragment-swap');
        swap.setAttribute('aria-pressed',String(showing.includes(alternate)));
        swap.addEventListener('click',()=>{showing=other;paint();figure.querySelector('.ps-fragment-swap')?.focus({preventScroll:true});});
        tools.append(swap);
      }
      // The source of the text shown: the place's own page in that witness, the place's page, or the witness.
      // A link says what it opens: the whole work, the page a quotation is taken from, or where the book can be found.
      const witness=work.witnesses?.[showing[0]];
      const [url,opens]=place.links?.[showing[0]]?[place.links[showing[0]],'work']:place.url?[place.url,place.opens??'book']:[witness?.url,witness?.opens??'work'];
      if(typeof url==='string'&&/^https:\/\//.test(url)){const link=el('a','text-button ps-fragment-whole',`↗ ${words({quotation:'fragmentQuoted',book:'fragmentBook'}[opens]??'fragmentWhole')}`);link.href=url;link.target='_blank';link.rel='noopener noreferrer';tools.append(link);}
      figure.append(tools);
    };
    paint();
  }
  // ── A place of a corpus work: the original's units from the corpus, the translation through the stand's alignment. ──
  const unitsFrom=async(work,versionId,first,last=first)=>{
    const found=[];let cursor=null;
    for(let round=0;round<4&&!found.some(unit=>unit.id===last);round++){
      const page=await provider.window(cursor?{documentId:work.documentId,versionId,cursor,limit:8}:{documentId:work.documentId,versionId,unitId:first,limit:8});
      for(const unit of page.units){if(unit.kind==='section'&&!unit.text)continue;found.push(unit);if(unit.id===last)break;}
      cursor=page.nextCursor;if(!cursor)break;
    }
    return found;
  };
  const overlaps=(a,b,s,e)=>a<e&&b>s;
  async function fillCorpus(figure,cite){
    if(!provider)throw new Error('No corpus provider');
    const data=await corpusAlignment(),work=data.works?.[cite.w];
    if(!work)throw new Error(`No alignment for ${cite.w}`);
    const [first,last]=cite.u;
    const originals=await unitsFrom(work,work.original.versionId,first,last);
    if(!originals.length)throw new Error(`No units for ${cite.w}`);
    const span=(unit,index)=>{const length=Array.from(unit.text).length;return [index===0&&Number.isInteger(cite.s)?cite.s:0,index===originals.length-1&&Number.isInteger(cite.e)?cite.e:length];};
    const available=[work.original.language,...work.translations.map(item=>item.language)];
    const {primary,alternate}=readingLanguages(locale(),work.original.language,available);
    const versionFor=language=>language===work.original.language?work.original:work.translations.find(item=>item.language===language&&originals.some(unit=>data.units?.[unit.id]?.t?.[item.versionId]));
    let showing=primary.filter(language=>versionFor(language));
    if(!showing.length)showing=[work.original.language];
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
        if(!found)for(const id of entry.u)if(!wanted.has(id))wanted.set(id,null);
      });
      const value=[];
      for(const [id,range] of wanted){
        const [unit]=await unitsFrom(work,version.versionId,id);if(!unit)continue;
        const chars=Array.from(unit.text);value.push({id,reference:unit.reference,text:(range?chars.slice(range[0],range[1]):chars).join('').trim()});
      }
      texts.set(language,value);return value;
    }
    const paint=async()=>{
      const parts=await Promise.all(showing.map(async language=>[language,await textOf(language)]));
      figure.replaceChildren();
      for(const [language,value] of parts){
        if(!value.length)continue;
        const version=versionFor(language);
        const quote=el('blockquote','ps-fragment-text');quote.lang=language==='grc'?'grc':language==='la'?'la':language;
        const {nodes}=clip(value.flatMap(item=>item.text.split(/\n{2,}/)).map(part=>el('p','ps-fragment-line',part.replace(/\s*\n\s*/g,' ').trim())).filter(node=>node.textContent));
        quote.append(...nodes);
        const references=[...new Set(value.map(item=>item.reference).filter(Boolean))];
        const caption=el('figcaption','ps-fragment-cite');
        caption.append(el('span','ps-fragment-source',[version?.label,references.length?references[0]+(references.length>1?`–${references.at(-1)}`:''):''].filter(Boolean).join(' · ')));
        figure.append(quote,caption);
      }
      const tools=el('div','ps-fragment-tools');
      if(alternate&&versionFor(alternate)){
        const back=primary.filter(language=>versionFor(language));
        const other=showing.includes(alternate)?back:[alternate];
        if(other.length&&other.join()!==showing.join()){
          const swap=button(showing.includes(alternate)?words('fragmentTranslation'):words('fragmentOriginal'),'text-button ps-fragment-swap');
          swap.setAttribute('aria-pressed',String(showing.includes(alternate)));
          swap.addEventListener('click',()=>{showing=other;void paint().then(()=>figure.querySelector('.ps-fragment-swap')?.focus({preventScroll:true})).catch(error=>onError?.(error));});
          tools.append(swap);
        }
      }
      if(openAddress){
        const shown=versionFor(showing[0])??work.original,unitId=shown===work.original?first:(texts.get(showing[0])??[])[0]?.id;
        if(unitId){const whole=button(`↗ ${words('fragmentWhole')}`,'text-button ps-fragment-whole');whole.addEventListener('click',()=>void openAddress({documentId:work.documentId,versionId:shown.versionId,unitId}).catch(error=>onError?.(error)));tools.append(whole);}
      }
      figure.append(tools);
    };
    await paint();
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
  function render(parent,value,{className='body',tag='p'}={}){
    const cites=citesOf(value),own=local(value).trim();
    if(!cites.length&&!own)return null;
    const box=el('div','ps-authored');
    for(const cite of cites)box.append(fragment(cite));
    if(own)box.append(el(tag,className,own));
    parent.append(box);
    return box;
  }
  return {render,fragment,showChapter,citesOf,hasAuthored:value=>hasAuthored(value,locale()),get reader(){return reader;},dispose(){reader?.dialog.remove();reader=null;}};
}

export {CHAPTER_KEYS,copyLocale};
