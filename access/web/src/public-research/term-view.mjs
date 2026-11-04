// Active terms: the first time a card or an article names a term of the stand's philological core
// (bonsai/<stand>/terms.json, bonsai/review/philology/README.md), the word opens on its origin. Pointing
// at it, or reaching it with the keyboard, shows the word of the original, the words of the translations
// shown beside it (one line per word, with the editions that use it), the Tree's own word where no
// translation shown says it or where it reads another, what the word does in the book and the
// place where the book first says it; opening it opens the book at that place. A finger's first touch
// shows the origin, and the origin's own button opens the book.

import {DEFAULT_LOCALE,localeText,localeValue,resolveLocale,shownLanguages} from './locale.mjs';

const WORDS={
  ru:{original:'В оригинале',tree:'В Древе',open:'Открыть первое место в книге',first:'Впервые'},
  en:{original:'In the original',tree:'In the Tree',open:'Open the first place in the book',first:'First said'},
};
const words=locale=>{const ru=resolveLocale(locale)==='ru';return ru?WORDS.ru:Object.fromEntries(Object.entries(WORDS.en).map(([key,value])=>[key,localeText(value,locale)]));};

// Text that is already an action, a title or a term keeps its own behaviour.
const SKIP='a,button,input,textarea,select,code,h1,summary,.ps-term,.ps-term-card,[data-terms="off"]';
const escape=value=>value.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');
const make=(tag,className,text)=>{const node=document.createElement(tag);if(className)node.className=className;if(text!==undefined)node.textContent=text;return node;};
const witnesses=rendering=>[].concat(rendering?.witness??[]);
const greekKey=value=>value.normalize('NFD').replace(/\p{M}/gu,'').toLowerCase().replaceAll('ς','σ');

/** The forms a language reads for each term: the Tree's words and the shown translation's words, and the original's in German. */
export function termForms(module,language,{edition=null}={}){
  const forms=new Map();
  for(const term of module?.terms??[]){
    const own=language===term.original?.lang||language==='de'&&(!term.original?.lang||term.original.lang==='de')?term.original?.forms??[]:[...(term.renderings?.[language]?.tos?.forms??[]),...witnesses(term.renderings?.[language]).filter(witness=>module.work!=='antiquity'||edition&&witness.edition===edition).flatMap(witness=>witness.forms??[])];
    for(const form of own)if(!forms.has(form))forms.set(form,term);
  }
  // A translation may use the Tree's word for a different term: its own
  // edition's reading takes precedence inside that quotation only.
  if(module?.work==='antiquity'&&edition)for(const term of module.terms??[]){
    for(const witness of witnesses(term.renderings?.[language]))if(witness.edition===edition)for(const form of witness.forms??[])forms.set(form,term);
  }
  return forms;
}

// The Tree's word in the reader's translation language when no translation shown says that word.
// Never a word of another language: a reader sees the original and their own translation only.
export function treeRendering(term,language){
  const own=term.renderings?.[language]?.tos?.term;if(!own)return null;
  const shown=witnesses(term.renderings?.[language]).map(witness=>witness?.term?.toLowerCase()).filter(Boolean);
  return shown.includes(own.toLowerCase())?null:{lang:language,term:own};
}

// One line for each word a language's translations use, with the editions that use it.
export function witnessLines(rendering,editions,locale){
  const lines=new Map();
  for(const witness of witnesses(rendering)){
    if(!witness?.edition||!witness.term)continue;
    const key=witness.term.toLowerCase();
    if(!lines.has(key))lines.set(key,{term:witness.term,short:[],full:[]});
    const line=lines.get(key),edition=editions?.[witness.edition];
    const short=localeValue(edition?.short,locale)||localeValue(edition?.label,locale)||witness.edition,full=localeValue(edition?.label,locale)||witness.edition;
    if(!line.short.includes(short))line.short.push(short);
    if(!line.full.includes(full))line.full.push(full);
  }
  return [...lines.values()].map(line=>({term:line.term,source:line.short.join(', '),title:line.full.join('\n')}));
}

// A form written with a capital (a German noun, Antonovsky's «Само», Common's "Superman") is read with
// that capital, so the verb "werden" stays apart from das Werden and «самого» from «Самого»; a form
// written in lower case is read in any case, so a sentence may begin with it.
const exact=form=>/^\p{Lu}/u.test(form);

// In English these two terms also have very frequent ordinary grammatical uses.
// Keep their noun phrases, including a bare heading, without marking the future
// auxiliary "will" or the verb "mean" throughout the reader.
export function termContextAllows(module,language,value,match,works=[]){
  if(module?.work!=='antiquity')return true;
  // Some everyday words name several different concepts. When an article has
  // a source work, only use those ambiguous renderings in works evidenced by
  // that term's cited uses; original-language forms remain available anywhere.
  if(['ru','en'].includes(language)&&works.length&&match.term?.contextWorks&&!works.some(work=>match.term.contextWorks.includes(work)))return false;
  if(language!=='en')return true;
  const word=match.text.toLowerCase();if(word!=='will'&&word!=='mean')return true;
  const before=value.slice(0,match.index),after=value.slice(match.index+match.text.length);
  if(!before.trim()&&!after.trim())return true;
  if(word==='will')return /(?:\b(?:the|a|his|her|my|your|our|their|its|one's|own|free)|[\p{L}][’']s)\s+$/iu.test(before);
  return /\b(?:the|a|its)\s+$/iu.test(before);
}

/** A matcher for whole-word forms, longest form first; `find(text)` lists the found forms in text order. */
export function termMatcher(forms,{greek=false}={}){
  if(!forms.size)return null;
  if(greek){
    const normalized=new Map();for(const [form,term] of forms)if(!normalized.has(greekKey(form)))normalized.set(greekKey(form),term);
    const regex=new RegExp(`(?<![\\p{L}\\p{N}])(?:${[...normalized.keys()].sort((a,b)=>b.length-a.length).map(escape).join('|')})(?![\\p{L}\\p{N}])`,'gu');
    return {find(value){
      let plain='',offset=0;const starts=[],ends=[];
      for(const char of value){const start=offset;offset+=char.length;const key=greekKey(char);if(!key&&ends.length)ends[ends.length-1]=offset;for(const part of key){plain+=part;starts.push(start);ends.push(offset);}}
      regex.lastIndex=0;const found=[];for(let match;(match=regex.exec(plain));){const start=starts[match.index],end=ends[match.index+match[0].length-1];found.push({index:start,text:value.slice(start,end),term:normalized.get(match[0])});}return found;
    }};
  }
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
export function createTerms({module,locale=()=>DEFAULT_LOCALE,open=()=>{}}={}){
  if(!module?.terms?.length)return {mark(){},dispose(){},get size(){return 0;}};
  const skip=module.work==='antiquity'?`${SKIP},figcaption,.eyebrow,.minor-title,.muted,.ps-source-status`:SKIP;
  const byId=new Map(module.terms.map(term=>[term.id,term]));
  const matchers=new Map();
  const matcherFor=(language,edition)=>{const key=`${language}:${edition??''}`;if(!matchers.has(key))matchers.set(key,termMatcher(termForms(module,language,{edition}),{greek:language==='grc'}));return matchers.get(key);};
  const languageOf=element=>{const lang=(element?.closest?.('[lang]')?.getAttribute('lang')||document.documentElement.lang||DEFAULT_LOCALE).toLowerCase();return ['grc','la','de','ru'].find(value=>lang===value||lang.startsWith(`${value}-`))??'en';};

  const observed=new Map();let marking=false;
  // A root does not observe its own removal. Watch the stable document for removed
  // subtrees and release only roots that have actually left the document.
  const removals=typeof MutationObserver==='function'?new MutationObserver(records=>{
    for(const record of records)for(const removed of record.removedNodes){
      for(const [root,observer] of observed){
        if(!root.isConnected&&(removed===root||removed.contains?.(root))){observer.disconnect();observed.delete(root);}
      }
    }
  }):null;
  removals?.observe(document.body,{childList:true,subtree:true});
  function mark(root){
    if(!root||marking)return;
    marking=true;
    try{
      // Once per term in each language of the root: a parallel reading opens the word in the original and in its translation.
      const seen=new Set([...root.querySelectorAll('.ps-term')].map(node=>`${languageOf(node)}:${node.dataset.term}`));
      const walker=document.createTreeWalker(root,NodeFilter.SHOW_TEXT,{acceptNode:node=>node.nodeValue.trim().length<3||node.parentElement?.closest(skip)?NodeFilter.FILTER_REJECT:NodeFilter.FILTER_ACCEPT});
      const queue=[];while(walker.nextNode())queue.push(walker.currentNode);
      while(queue.length){
        const node=queue.shift(),language=languageOf(node.parentElement),edition=node.parentElement?.closest?.('[data-edition]')?.dataset.edition,matcher=matcherFor(language,edition);if(!matcher)continue;
        const works=(node.parentElement?.closest?.('[data-term-works]')?.dataset.termWorks??'').split(' ').filter(Boolean);
        const found=matcher.find(node.nodeValue).find(item=>!seen.has(`${language}:${item.term.id}`)&&termContextAllows(module,language,node.nodeValue,item,works));
        if(!found)continue;
        seen.add(`${language}:${found.term.id}`);
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
  if(module.work==='antiquity')card.dataset.stand='antiquity';
  card.id=`ps-term-card-${Math.random().toString(36).slice(2,8)}`;document.body.append(card);
  let shownFor=null,showTimer=0,hideTimer=0;
  function paint(term){
    // The original and the reader's one translation (shownLanguages), never the other translations.
    const [,ui]=shownLanguages(locale(),term.original?.lang??'de',Object.keys(term.renderings??{})),w=words(locale()),editions=module.editions??{};
    const rows=[];
    const head=make('p','ps-term-original');head.append(make('span','ps-term-eyebrow',w.original),Object.assign(make('span','ps-term-lemma',term.original?.lemma??''),{lang:term.original?.lang??'de'}));rows.push(head);
    for(const language of [ui].filter(Boolean)){
      for(const witness of witnessLines(term.renderings?.[language],editions,locale())){
        const line=make('p','ps-term-witness');const said=make('span','ps-term-word',witness.term);said.lang=language;const source=make('span','ps-term-source',witness.source);source.title=witness.title;line.append(said,source);rows.push(line);}
    }
    const own=ui?treeRendering(term,ui):null;
    if(own){const line=make('p','ps-term-witness is-tree');const said=make('span','ps-term-word',own.term);said.lang=own.lang;line.append(said,make('span','ps-term-source',w.tree));rows.push(line);}
    const gloss=localeValue(term.gloss,locale());if(gloss)rows.push(make('p','ps-term-gloss',gloss));
    if(term.first?.q){const first=make('p','ps-term-first');first.append(make('span','ps-term-eyebrow',w.first),Object.assign(make('q','',term.first.q),{lang:term.original?.lang??'de'}));rows.push(first);}
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
    // A modal reading sits in the top layer; the card joins it there so it shows above the book.
    const layer=span.closest('dialog[open]')??document.body;if(card.parentElement!==layer)layer.append(card);
    shownFor=span;span.setAttribute('aria-expanded','true');span.setAttribute('aria-controls',card.id);card.hidden=false;place(span);
  }
  function hide(now=false){
    clearTimeout(showTimer);clearTimeout(hideTimer);
    const done=()=>{card.hidden=true;shownFor?.removeAttribute('aria-expanded');shownFor=null;};
    if(now)done();else hideTimer=setTimeout(done,160);
  }
  const termOf=event=>event.target?.closest?.('.ps-term');
  let touch=false,touchPreview=null;
  const over=event=>{const span=termOf(event);if(!span||event.pointerType==='touch')return;clearTimeout(showTimer);showTimer=setTimeout(()=>show(span),220);};
  const out=event=>{if(termOf(event)&&!card.contains(event.relatedTarget))hide();};
  const down=event=>{touch=event.pointerType==='touch';touchPreview=touch&&!card.hidden?shownFor:null;if(!card.hidden&&!card.contains(event.target)&&!termOf(event))hide(true);};
  const click=event=>{
    const span=termOf(event);if(!span)return;event.preventDefault();event.stopPropagation();
    // A finger first sees the origin; the origin's button opens the book.
    if(touch&&touchPreview!==span){touch=false;touchPreview=null;show(span);return;}
    touch=false;touchPreview=null;
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
      removals?.disconnect();for(const observer of observed.values())observer.disconnect();observed.clear();card.remove();
      document.removeEventListener('pointerover',over);document.removeEventListener('pointerout',out);document.removeEventListener('pointerdown',down,true);
      document.removeEventListener('click',click,true);document.removeEventListener('keydown',key,true);document.removeEventListener('focusin',focus);document.removeEventListener('focusout',blur);
      removeEventListener('scroll',scrolled,{capture:true});
    },
  };
}
