// The stand's quotations, its parallel reading and the sheets they open offer the
// origin of a selected word of the book, where the stand's word layer reads that
// word: a small menu beside the word, then the origin in a sheet. A word that
// belongs to a term also offers how the translations render it, here and, through
// `readerActions`, in the reader.
import {createSelectionMenu} from '../corpus-reader/selection-menu.mjs';
import {renderOrigin} from '../corpus-reader/word-origin.mjs';
import {bonsaiWordByForm} from '../corpus-reader/bonsai-words.mjs';
import {copyLocale,localeText} from './locale.mjs';
import {renderRenderings,renderingsWord,translationLanguage,wordRenderings} from './renderings-view.mjs';
import '../corpus-reader/philology.css';

const LANGUAGES=new Set(['de','ru','en']);

/** Where the selection starts in the unit's display text: the line's offset and the text before the selection. */
function displayPosition(element,range){
  const before=document.createRange();
  before.selectNodeContents(element);before.setEnd(range.startContainer,range.startOffset);
  return Number(element.dataset.offset||0)+Array.from(before.toString()).length;
}

/**
 * Offer the origin of words selected inside `root` in elements carrying `data-chapter` and `data-unit`
 * (a witness unit) within an element whose `lang` names the witness. `openPlace(place)` opens a place of the book.
 * With the stand's `renderings` and `terms`, a word of a term also offers its renderings; `fragment(cite)` shows their places.
 * Each action is offered where the word layer reads the selected word.
 */
export function createStandOrigin({root,locale,openPlace=null,renderings=null,terms=null,fragment=null}){
  let dialog=null;
  const label=()=>copyLocale(locale())==='en'?localeText('Origin of the word',locale()):'Происхождение слова';
  function sheet(){
    if(dialog?.isConnected)return dialog;
    dialog=document.createElement('dialog');dialog.className='ps-origin-sheet cr-inspector';
    const close=document.createElement('button');close.type='button';close.className='ps-origin-close';close.textContent='×';
    close.setAttribute('aria-label',copyLocale(locale())==='en'?localeText('Close',locale()):'Закрыть');
    close.addEventListener('click',()=>dialog.close());
    const body=document.createElement('div');body.className='cr-inspector-body ps-origin-body';
    dialog.append(close,body);dialog.addEventListener('click',event=>{if(event.target===dialog)dialog.close();});
    (root.ownerDocument?.body??document.body).append(dialog);
    return dialog;
  }
  // The selected word as the word layer reads it; the menu and the sheet ask once for the same word.
  const resolved=new Map();
  function word({text,range,element}){
    const unitNode=element.closest('[data-unit][data-chapter]'),language=element.closest('[lang]')?.lang;
    if(!unitNode||!LANGUAGES.has(language))return Promise.resolve(null);
    const near=displayPosition(unitNode,range),key=[unitNode.dataset.chapter,unitNode.dataset.unit,language,text,near].join('|');
    if(!resolved.has(key)){
      if(resolved.size>64)resolved.clear();
      resolved.set(key,bonsaiWordByForm({chapter:unitNode.dataset.chapter,language,unitId:unitNode.dataset.unit,form:text,near}).catch(()=>null).then(context=>({language,context})));
    }
    return resolved.get(key);
  }
  function open(){
    const view=sheet(),body=view.querySelector('.ps-origin-body');
    body.replaceChildren();
    if(!view.open)view.showModal();
    return {view,body};
  }
  async function show(value){
    const found=await word(value);if(!found)return;
    const {view,body}=open();
    await renderOrigin(body,{locale:locale(),form:value.text,language:found.language,context:found.context,
      onOpenPlace:openPlace?place=>{view.close();openPlace(place);}:undefined});
  }
  // The terms a word read by the layer belongs to, with a rendering in the translation shown beside it.
  const translationFor=language=>language==='de'?translationLanguage(locale()):language;
  const termsFor=found=>found?.language?wordRenderings(renderings,terms,found).filter(entry=>entry[translationFor(found.language)]):[];
  const known=value=>word(value).then(found=>{
    const lemma=found?.context?.words?.[0]?.lemma;
    return lemma?{language:found.language,form:value.text,lemma}:null;
  });
  function showRenderingsFor(found){
    const entries=termsFor(found);if(!entries.length)return;
    const {body}=open();
    body.append(Object.assign(document.createElement('h2'),{className:'cr-inspector-title',textContent:renderingsWord('translations',locale())}));
    for(const entry of entries)renderRenderings(body,entry,{language:translationFor(found.language),locale:locale(),fragment});
  }
  const translations=renderings&&fragment?{get label(){return renderingsWord('translations',locale());},
    when:(value,found)=>Promise.resolve(found??known(value)).then(found=>termsFor(found).length>0),
    run:(value,found)=>void Promise.resolve(found??known(value)).then(showRenderingsFor)}:null;
  const menu=createSelectionMenu({container:root,accept:value=>Boolean(value.element.closest('[data-unit][data-chapter]')),
    actions:[{get label(){return label();},when:value=>known(value).then(Boolean),run:value=>void show(value)},...(translations?[translations]:[])]});
  return {
    /** The actions the reader adds for a word its layer reads: how the translations render the word's term. */
    readerActions:translations?[translations]:[],
    dispose(){menu.dispose();dialog?.remove();dialog=null;},
  };
}
