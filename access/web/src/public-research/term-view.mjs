// Active terms: the first time a card, an article or the book names a term of the stand that has a node of
// its own (a concept or a figure of the core, an entry of the library), the word is a link to that node.
// Pointing at it, or reaching it with the keyboard, shows where it leads: the node's kind, its title and its
// opening line; following it opens the node. A finger's first touch shows the node, and the card's own button
// opens it. A term without a node stays plain text, and a word never links the node whose card already holds it.
// The words of the original and of the translations, with their first place in the book, belong to the
// philological core (bonsai/<stand>/terms.json, bonsai/review/philology/README.md); the helpers below read them.

import {DEFAULT_LOCALE,localeText,localeValue,resolveLocale} from './locale.mjs';

const WORDS={
  ru:{open:'Перейти к узлу'},
  en:{open:'Open node'},
};
const words=locale=>{const ru=resolveLocale(locale)==='ru';return ru?WORDS.ru:Object.fromEntries(Object.entries(WORDS.en).map(([key,value])=>[key,localeText(value,locale)]));};

// Text that is already an action, a title, a label or a term keeps its own behaviour.
const SKIP='a,button,input,textarea,select,code,h1,h2,h3,summary,figcaption,.minor-title,.rb-field-label,.ps-term,.ps-term-card,[data-terms="off"]';
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
 * The node a stand's term names. In the Antiquity stand: the concept of the library when the term has one, else
 * the term's own entry in the library. In Zarathustra: the concept or the figure of the core the term names.
 * `has(id)` tells whether the stand holds that node.
 */
export function termNodes({stand,materialIds={},has=()=>false}={}){
  return term=>{const ids=stand==='antiquity'?[term?.concept&&`antiquity:catalog:${term.concept}`,`antiquity:catalog:lexical:${term?.id}`]:[term?.concept&&materialIds?.[term.concept]];return ids.find(id=>id&&has(id))??null;};
}

/** The terms that lead to a node, and the node each one leads to. */
export function linkedTerms(module,nodeOf=()=>null){
  const nodes=new Map();
  for(const term of module?.terms??[]){const id=nodeOf(term);if(id)nodes.set(term.id,id);}
  return {module:{...module,terms:(module?.terms??[]).filter(term=>nodes.has(term.id))},nodes};
}

/**
 * Links from a stand's words to its nodes. `nodeOf(term)` gives the node a term names, or nothing;
 * `describe(id)` gives that node's kind, title and opening line in the reader's language; `hrefOf(id)` its
 * address; `openNode(id)` opens it. `mark(root)` makes the first mention of every node in `root` a link
 * (once per root, again after content arrives later), except where the root is that node's own card.
 */
export function createTerms({module,locale=()=>DEFAULT_LOCALE,nodeOf=()=>null,describe=()=>null,hrefOf=()=>null,openNode=()=>{}}={}){
  const {module:linked,nodes}=linkedTerms(module,nodeOf);
  if(!linked.terms.length)return {mark(){},dispose(){},get size(){return 0;}};
  const skip=linked.work==='antiquity'?`${SKIP},figcaption,.eyebrow,.minor-title,.muted,.ps-source-status`:SKIP;
  const matchers=new Map();
  const matcherFor=(language,edition)=>{const key=`${language}:${edition??''}`;if(!matchers.has(key))matchers.set(key,termMatcher(termForms(linked,language,{edition}),{greek:language==='grc'}));return matchers.get(key);};
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
      // Once per node in each language of the root: a parallel reading links the word in the original and in its translation.
      const seen=new Set([...root.querySelectorAll('.ps-term')].map(node=>`${languageOf(node)}:${node.dataset.node}`));
      const walker=document.createTreeWalker(root,NodeFilter.SHOW_TEXT,{acceptNode:node=>node.nodeValue.trim().length<3||node.parentElement?.closest(skip)?NodeFilter.FILTER_REJECT:NodeFilter.FILTER_ACCEPT});
      const queue=[];while(walker.nextNode())queue.push(walker.currentNode);
      while(queue.length){
        const node=queue.shift(),parent=node.parentElement,language=languageOf(parent),edition=parent?.closest?.('[data-edition]')?.dataset.edition,matcher=matcherFor(language,edition);if(!matcher)continue;
        const works=(parent?.closest?.('[data-term-works]')?.dataset.termWorks??'').split(' ').filter(Boolean);
        // The card of a node does not link to itself.
        const here=parent?.closest?.('[data-term-node]')?.dataset.termNode??'';
        const found=matcher.find(node.nodeValue).find(item=>{const target=nodes.get(item.term.id);return target!==here&&!seen.has(`${language}:${target}`)&&termContextAllows(linked,language,node.nodeValue,item,works);});
        if(!found)continue;
        const target=nodes.get(found.term.id);seen.add(`${language}:${target}`);
        const start=found.index,end=start+found.text.length,rest=node.splitText(start).splitText(end-start);
        const word=node.nextSibling,href=hrefOf(target),link=make(href?'a':'span','ps-term');
        if(href)link.href=href;else{link.tabIndex=0;link.setAttribute('role','link');}
        link.dataset.term=found.term.id;link.dataset.node=target;
        word.replaceWith(link);link.append(word);queue.unshift(rest);
      }
    }finally{marking=false;}
    // Witness texts arrive after the card: a root is read again when content comes into it.
    if(!observed.has(root)&&typeof MutationObserver==='function'){
      let frame=0;const observer=new MutationObserver(()=>{if(marking||frame)return;frame=requestAnimationFrame(()=>{frame=0;if(root.isConnected)mark(root);else{observer.disconnect();observed.delete(root);}});});
      observer.observe(root,{childList:true,subtree:true});observed.set(root,observer);
    }
  }

  // The node's card: one for the page, beside the word that leads to it.
  const card=make('section','ps-term-card');card.hidden=true;card.setAttribute('role','dialog');card.setAttribute('aria-modal','false');
  if(linked.work==='antiquity')card.dataset.stand='antiquity';
  card.id=`ps-term-card-${Math.random().toString(36).slice(2,8)}`;document.body.append(card);
  let shownFor=null,showTimer=0,hideTimer=0;
  function paint(span){
    const id=span.dataset.node,node=describe(id);if(!node)return false;
    const kind=make('p','ps-term-kind');const star=make('span','ps-term-star');star.setAttribute('aria-hidden','true');kind.append(star,make('span','',node.kind??''));
    const rows=[kind,make('p','ps-term-title',node.title??'')];
    if(node.lead)rows.push(make('p','ps-term-lead',node.lead));
    const go=make('button','ps-term-open',words(locale()).open);go.type='button';go.addEventListener('click',()=>{hide(true);openNode(id);});rows.push(go);
    card.replaceChildren(...rows);card.setAttribute('aria-label',node.title??'');return true;
  }
  function place(span){
    const r=span.getBoundingClientRect(),width=Math.min(320,innerWidth-16);card.style.width=`${width}px`;
    const left=Math.max(8,Math.min(innerWidth-width-8,r.left+r.width/2-width/2));card.style.left=`${Math.round(left)}px`;
    const height=card.offsetHeight,below=r.bottom+8+height<=innerHeight-8;
    card.style.top=`${Math.round(below?r.bottom+8:Math.max(8,r.top-8-height))}px`;
  }
  const release=span=>{if(!span)return;delete span.dataset.shown;span.removeAttribute('aria-describedby');};
  function show(span){
    clearTimeout(hideTimer);
    if(shownFor!==span){if(!paint(span))return;release(shownFor);}
    // A modal reading sits in the top layer; the card joins it there so it shows above the book.
    const layer=span.closest('dialog[open]')??document.body;if(card.parentElement!==layer)layer.append(card);
    shownFor=span;span.dataset.shown='true';span.setAttribute('aria-describedby',card.id);card.hidden=false;place(span);
  }
  function hide(now=false){
    clearTimeout(showTimer);clearTimeout(hideTimer);
    const done=()=>{card.hidden=true;release(shownFor);shownFor=null;};
    if(now)done();else hideTimer=setTimeout(done,160);
  }
  const follow=span=>{hide(true);if(span?.dataset.node)openNode(span.dataset.node);};
  const termOf=event=>event.target?.closest?.('.ps-term');
  let touch=false,touchPreview=null;
  const over=event=>{const span=termOf(event);if(!span||event.pointerType==='touch')return;clearTimeout(showTimer);showTimer=setTimeout(()=>show(span),220);};
  const out=event=>{if(termOf(event)&&!card.contains(event.relatedTarget))hide();};
  const down=event=>{touch=event.pointerType==='touch';touchPreview=touch&&!card.hidden?shownFor:null;if(!card.hidden&&!card.contains(event.target)&&!termOf(event))hide(true);};
  const click=event=>{
    const span=termOf(event);if(!span)return;
    // A link opened in another tab or window keeps the browser's own way.
    if(span.href&&(event.metaKey||event.ctrlKey||event.shiftKey||event.altKey||event.button>0))return;
    event.preventDefault();event.stopPropagation();
    // A finger first sees where the word leads; the card's button, or a second touch, follows it.
    if(touch&&touchPreview!==span){touch=false;touchPreview=null;show(span);return;}
    touch=false;touchPreview=null;follow(span);
  };
  const key=event=>{
    // Escape closes the card and only the card: whatever stands beneath it stays.
    if(event.key==='Escape'&&!card.hidden){event.preventDefault();hide(true);return;}
    // A real link follows on Enter by itself, through its click.
    const span=termOf(event);if(!span||span.href)return;
    if(event.key==='Enter'){event.preventDefault();follow(span);}
  };
  const focus=event=>{const span=termOf(event);if(span)show(span);};
  const blur=event=>{if(termOf(event)&&!card.contains(event.relatedTarget))hide();};
  card.addEventListener('pointerenter',()=>clearTimeout(hideTimer));card.addEventListener('pointerleave',()=>hide());card.addEventListener('focusout',event=>{if(!card.contains(event.relatedTarget)&&!event.relatedTarget?.closest?.('.ps-term'))hide();});
  document.addEventListener('pointerover',over);document.addEventListener('pointerout',out);document.addEventListener('pointerdown',down,true);
  document.addEventListener('click',click,true);document.addEventListener('keydown',key,true);document.addEventListener('focusin',focus);document.addEventListener('focusout',blur);
  const scrolled=()=>{if(!card.hidden)hide(true);};addEventListener('scroll',scrolled,{passive:true,capture:true});
  return {
    mark,
    get size(){return nodes.size;},
    dispose(){
      removals?.disconnect();for(const observer of observed.values())observer.disconnect();observed.clear();card.remove();
      document.removeEventListener('pointerover',over);document.removeEventListener('pointerout',out);document.removeEventListener('pointerdown',down,true);
      document.removeEventListener('click',click,true);document.removeEventListener('keydown',key,true);document.removeEventListener('focusin',focus);document.removeEventListener('focusout',blur);
      removeEventListener('scroll',scrolled,{capture:true});
    },
  };
}
