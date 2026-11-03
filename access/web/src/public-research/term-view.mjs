// Active terms: the first time a card or an article names a term of the stand's philological core
// (bonsai/<stand>/terms.json, bonsai/review/philology/README.md), the word opens on its origin. Pointing
// at it, or reaching it with the keyboard, shows the word of the original, the words of the translations
// shown beside it, the Tree's own word where it reads another, what the word does in the book and the
// place where the book first says it; opening it opens the book at that place. A finger's first touch
// shows the origin, and the origin's own button opens the book.

import {localeText,localeValue,resolveLocale} from './locale.mjs';

const WORDS={
  ru:{original:'В оригинале',tree:'В Древе',open:'Открыть первое место в книге',first:'Впервые'},
  en:{original:'In the original',tree:'In the Tree',open:'Open the first place in the book',first:'First said'},
};
const words=locale=>{const ru=resolveLocale(locale)==='ru';return ru?WORDS.ru:Object.fromEntries(Object.entries(WORDS.en).map(([key,value])=>[key,localeText(value,locale)]));};

// Text that is already an action, a title or a term keeps its own behaviour.
const SKIP='a,button,input,textarea,select,code,h1,summary,.ps-term,.ps-term-card,[data-terms="off"]';
const escape=value=>value.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');
const make=(tag,className,text)=>{const node=document.createElement(tag);if(className)node.className=className;if(text!==undefined)node.textContent=text;return node;};

/** The forms a language reads for each term: the Tree's words and the shown translation's words, and the original's in German. */
export function termForms(module,language){
  const forms=new Map();
  for(const term of module?.terms??[]){
    const own=language==='de'?term.original?.forms??[]:[...(term.renderings?.[language]?.tos?.forms??[]),...(term.renderings?.[language]?.witness?.forms??[])];
    for(const form of own)if(!forms.has(form))forms.set(form,term);
  }
  return forms;
}

// A form written with a capital (a German noun, Antonovsky's «Само», Common's "Superman") is read with
// that capital, so the verb "werden" stays apart from das Werden and «самого» from «Самого»; a form
// written in lower case is read in any case, so a sentence may begin with it.
const exact=form=>/^\p{Lu}/u.test(form);

/** A matcher for whole-word forms, longest form first; `find(text)` lists the found forms in text order. */
export function termMatcher(forms){
  if(!forms.size)return null;
  const byCase=new Map(),anyCase=new Map();
  for(const [form,term]of forms){if(exact(form))byCase.set(form,term);else if(!anyCase.has(form.toLowerCase()))anyCase.set(form.toLowerCase(),term);}
  const pattern=(keys,flags)=>keys.length?new RegExp(`(?<![\\p{L}\\p{N}])(?:${keys.sort((a,b)=>b.length-a.length).map(escape).join('|')})(?![\\p{L}\\p{N}])`,flags):null;
  const patterns=[[pattern([...byCase.keys()],'gu'),text=>byCase.get(text)],[pattern([...anyCase.keys()],'giu'),text=>anyCase.get(text.toLowerCase())]].filter(([regex])=>regex);
  return {find(text){
    const found=[];
    for(const [regex,termOf]of patterns){regex.lastIndex=0;for(let match;(match=regex.exec(text));)found.push({index:match.index,text:match[0],term:termOf(match[0])});}
    return found.filter(item=>item.term).sort((a,b)=>a.index-b.index||b.text.length-a.text.length);
  }};
}

/**
 * Terms for a stand. `mark(root)` makes the first mention of every term in `root` active (once per root,
 * again after content arrives later); `open(cite)` opens the book at a place.
 */
export function createTerms({module,locale=()=> 'ru',open=()=>{}}={}){
  if(!module?.terms?.length)return {mark(){},dispose(){},get size(){return 0;}};
  const byId=new Map(module.terms.map(term=>[term.id,term]));
  const matchers=new Map();
  const matcherFor=language=>{if(!matchers.has(language))matchers.set(language,termMatcher(termForms(module,language)));return matchers.get(language);};
  const languageOf=element=>{const lang=(element?.closest?.('[lang]')?.getAttribute('lang')||document.documentElement.lang||'ru').toLowerCase();return lang.startsWith('de')?'de':lang.startsWith('ru')?'ru':'en';};

  const observed=new Map();let marking=false;
  function mark(root){
    if(!root||marking)return;
    marking=true;
    try{
      const seen=new Set([...root.querySelectorAll('.ps-term')].map(node=>node.dataset.term));
      const walker=document.createTreeWalker(root,NodeFilter.SHOW_TEXT,{acceptNode:node=>node.nodeValue.trim().length<3||node.parentElement?.closest(SKIP)?NodeFilter.FILTER_REJECT:NodeFilter.FILTER_ACCEPT});
      const queue=[];while(walker.nextNode())queue.push(walker.currentNode);
      while(queue.length){
        const node=queue.shift(),matcher=matcherFor(languageOf(node.parentElement));if(!matcher)continue;
        const found=matcher.find(node.nodeValue).find(item=>!seen.has(item.term.id));
        if(!found)continue;
        seen.add(found.term.id);
        const start=found.index,end=start+found.text.length,rest=node.splitText(start).splitText(end-start);
        const word=node.nextSibling,span=make('span','ps-term');
        span.dataset.term=found.term.id;span.tabIndex=0;span.setAttribute('role','button');span.setAttribute('aria-haspopup','dialog');
        word.replaceWith(span);span.append(word);queue.unshift(rest);
      }
    }finally{marking=false;}
    // Witness texts arrive after the card: a root is read again when content comes into it.
    if(!observed.has(root)&&typeof MutationObserver==='function'){
      let frame=0;const observer=new MutationObserver(()=>{if(marking||frame)return;frame=requestAnimationFrame(()=>{frame=0;if(root.isConnected)mark(root);else{observer.disconnect();observed.delete(root);}});});
      observer.observe(root,{childList:true,subtree:true});observed.set(root,observer);
    }
  }

  // The origin card: one for the page, beside the word it shows.
  const card=make('section','ps-term-card');card.hidden=true;card.setAttribute('role','dialog');card.setAttribute('aria-modal','false');
  card.id=`ps-term-card-${Math.random().toString(36).slice(2,8)}`;document.body.append(card);
  let shownFor=null,showTimer=0,hideTimer=0;
  function paint(term){
    const w=words(locale()),ui=resolveLocale(locale())==='ru'?'ru':'en',editions=module.editions??{};
    const label=edition=>localeValue(editions[edition]?.label,locale())||edition;
    const rows=[];
    const head=make('p','ps-term-original');head.append(make('span','ps-term-eyebrow',w.original),Object.assign(make('span','ps-term-lemma',term.original?.lemma??''),{lang:'de'}));rows.push(head);
    for(const language of ['ru','en']){
      const witness=term.renderings?.[language]?.witness;if(!witness?.edition)continue;
      const line=make('p','ps-term-witness');const said=make('span','ps-term-word',witness.term);said.lang=language;line.append(said,make('span','ps-term-source',label(witness.edition)));rows.push(line);
    }
    const own=term.renderings?.[ui]?.tos?.term,shown=term.renderings?.[ui]?.witness?.term;
    if(own&&shown&&own.toLowerCase()!==shown.toLowerCase()){const line=make('p','ps-term-witness is-tree');const said=make('span','ps-term-word',own);said.lang=ui;line.append(said,make('span','ps-term-source',w.tree));rows.push(line);}
    const gloss=localeValue(term.gloss,locale());if(gloss)rows.push(make('p','ps-term-gloss',gloss));
    if(term.first?.q){const first=make('p','ps-term-first');first.append(make('span','ps-term-eyebrow',w.first),Object.assign(make('q','',term.first.q),{lang:'de'}));rows.push(first);}
    const go=make('button','ps-term-open',w.open);go.type='button';go.addEventListener('click',()=>{hide(true);open(term.first);});rows.push(go);
    card.replaceChildren(...rows);card.setAttribute('aria-label',term.original?.lemma??term.id);
  }
  function place(span){
    const r=span.getBoundingClientRect(),width=Math.min(320,innerWidth-16);card.style.width=`${width}px`;
    const left=Math.max(8,Math.min(innerWidth-width-8,r.left+r.width/2-width/2));card.style.left=`${Math.round(left)}px`;
    const height=card.offsetHeight,below=r.bottom+8+height<=innerHeight-8;
    card.style.top=`${Math.round(below?r.bottom+8:Math.max(8,r.top-8-height))}px`;
  }
  function show(span){
    clearTimeout(hideTimer);const term=byId.get(span.dataset.term);if(!term)return;
    if(shownFor!==span){shownFor?.removeAttribute('aria-expanded');paint(term);}
    shownFor=span;span.setAttribute('aria-expanded','true');span.setAttribute('aria-controls',card.id);card.hidden=false;place(span);
  }
  function hide(now=false){
    clearTimeout(showTimer);clearTimeout(hideTimer);
    const done=()=>{card.hidden=true;shownFor?.removeAttribute('aria-expanded');shownFor=null;};
    if(now)done();else hideTimer=setTimeout(done,160);
  }
  const termOf=event=>event.target?.closest?.('.ps-term');
  let touch=false;
  const over=event=>{const span=termOf(event);if(!span||event.pointerType==='touch')return;clearTimeout(showTimer);showTimer=setTimeout(()=>show(span),220);};
  const out=event=>{if(termOf(event)&&!card.contains(event.relatedTarget))hide();};
  const down=event=>{touch=event.pointerType==='touch';if(!card.hidden&&!card.contains(event.target)&&!termOf(event))hide(true);};
  const click=event=>{
    const span=termOf(event);if(!span)return;event.preventDefault();event.stopPropagation();
    // A finger first sees the origin; the origin's button opens the book.
    if(touch&&shownFor!==span){show(span);return;}
    const term=byId.get(span.dataset.term);hide(true);if(term?.first)open(term.first);
  };
  const key=event=>{
    if(event.key==='Escape'&&!card.hidden){hide(true);return;}
    const span=termOf(event);if(!span)return;
    if(event.key==='Enter'||event.key===' '){event.preventDefault();const term=byId.get(span.dataset.term);hide(true);if(term?.first)open(term.first);}
  };
  const focus=event=>{const span=termOf(event);if(span)show(span);};
  const blur=event=>{if(termOf(event)&&!card.contains(event.relatedTarget))hide();};
  card.addEventListener('pointerenter',()=>clearTimeout(hideTimer));card.addEventListener('pointerleave',()=>hide());card.addEventListener('focusout',event=>{if(!card.contains(event.relatedTarget)&&!event.relatedTarget?.closest?.('.ps-term'))hide();});
  document.addEventListener('pointerover',over);document.addEventListener('pointerout',out);document.addEventListener('pointerdown',down,true);
  document.addEventListener('click',click,true);document.addEventListener('keydown',key,true);document.addEventListener('focusin',focus);document.addEventListener('focusout',blur);
  const scrolled=()=>{if(!card.hidden)hide(true);};addEventListener('scroll',scrolled,{passive:true,capture:true});
  return {
    mark,
    get size(){return byId.size;},
    dispose(){
      for(const observer of observed.values())observer.disconnect();observed.clear();card.remove();
      document.removeEventListener('pointerover',over);document.removeEventListener('pointerout',out);document.removeEventListener('pointerdown',down,true);
      document.removeEventListener('click',click,true);document.removeEventListener('keydown',key,true);document.removeEventListener('focusin',focus);document.removeEventListener('focusout',blur);
      removeEventListener('scroll',scrolled,{capture:true});
    },
  };
}
