// How a translation renders a term of the book, place by place (bonsai/<stand>/renderings.json,
// bonsai/review/philology/README.md): one band per word, as wide as its places, and under each word the
// sentences where it stands. A term's node shows it in its own tab; a word of the book selected in the
// stand opens it in a sheet.
import {localeValue,resolveLocale,shownLanguages} from './locale.mjs';
import witnessManifest from '../../../../bonsai/zarathustra/witnesses.json' with {type:'json'};
import './renderings-view.css';

const LANGS=['ru','en','es','de','zh-Hans'];
const WORDS={
  translations:['Переводы','Translations','Traducciones','Übersetzungen','译本'],
  otherwise:['иначе','otherwise','de otro modo','anders','其他译法'],
};
const make=(tag,className,text)=>{const node=document.createElement(tag);if(className)node.className=className;if(text!==undefined)node.textContent=text;return node;};

/** A word of this view in the reader's language. */
export const renderingsWord=(key,locale)=>WORDS[key]?.[Math.max(0,LANGS.indexOf(resolveLocale(locale)))]??key;

/** The translation a reader reads beside the original: their own language's, else English. */
export const translationLanguage=locale=>shownLanguages(locale,'de',['de','en','ru'])[1]??'en';

/** The terms a node of the core names. */
export const conceptRenderings=(renderings,concept)=>Object.values(renderings?.terms??{}).filter(entry=>entry.concept===concept);

/**
 * The terms a word of the book belongs to: in the original by its form (or lemma), in a translation by its
 * lemma among the words that translation uses for the term.
 */
export function wordRenderings(renderings,terms,{language,form,lemma}){
  const key=value=>String(value??'').toLocaleLowerCase(language);
  const forms=new Map((terms?.terms??[]).map(term=>[term.id,new Set([...(term.original?.forms??[]),term.original?.lemma].filter(Boolean).map(key))]));
  return Object.entries(renderings?.terms??{}).filter(([id,entry])=>language==='de'
    ?[form,lemma].some(value=>value&&forms.get(id)?.has(key(value)))
    :Boolean(lemma)&&(entry[language]?.words??[]).some(([word])=>key(word)===key(lemma))).map(([,entry])=>entry);
}

/** Append how `language`'s translation renders the term `entry`; `fragment(cite)` shows a place. False when it has nothing to show. */
export function renderRenderings(parent,entry,{language,locale,fragment}){
  const rendering=entry?.[language];
  const groups=[...(rendering?.words??[]),...(rendering?.other?.length?[[renderingsWord('otherwise',locale),rendering.other]]:[])].filter(([,places])=>places?.length);
  if(!groups.length)return false;
  const part=make('section','ps-core-section ps-renderings');
  part.append(make('h3','minor-title',entry.lemma));
  const short=localeValue(witnessManifest.witnesses?.[language]?.short,locale);if(short)part.append(make('p','muted',short));
  const bar=make('div','ps-renderings-bar');bar.setAttribute('aria-hidden','true');
  groups.forEach(([word,places],index)=>{const piece=make('span','',word);piece.style.flexGrow=String(places.length);piece.dataset.rank=String(Math.min(index,3));bar.append(piece);});
  part.append(bar);
  for(const [word,places] of groups){
    const box=make('details','ps-core-fold'),body=make('div');box.append(make('summary','',`${word} · ${places.length}`),body);
    box.addEventListener('toggle',()=>{if(box.open&&!body.childElementCount)for(const [c,unit,s,e] of places)body.append(fragment({w:'also-sprach-zarathustra',c,u:[unit,unit],s,e}));});
    part.append(box);
  }
  parent.append(part);
  return true;
}
