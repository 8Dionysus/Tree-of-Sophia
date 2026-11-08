// The stand's quotations and parallel reading offer the origin of a selected
// word of the book: a small menu beside the word, then the origin in a sheet.
// A word that belongs to a term also offers how the translations render it.
import {createSelectionMenu} from '../corpus-reader/selection-menu.mjs';
import {renderOrigin} from '../corpus-reader/word-origin.mjs';
import {bonsaiWordByForm} from '../corpus-reader/bonsai-words.mjs';
import {copyLocale,localeText} from './locale.mjs';
import {renderRenderings,renderingsWord,translationLanguage,wordRenderings} from './renderings-view.mjs';
import {benchListens,layOnBench} from './bench/bridge.mjs';
import '../corpus-reader/philology.css';

const LANGUAGES=new Set(['de','ru','en']);

/** Where a point of the selection stands in the unit's display text: the line's offset and the text before it. */
function displayPosition(element,range,{end=false}={}){
  const before=document.createRange();
  before.selectNodeContents(element);
  if(end)before.setEnd(range.endContainer,range.endOffset);else before.setEnd(range.startContainer,range.startOffset);
  return Number(element.dataset.offset||0)+Array.from(before.toString()).length;
}
/** A passage of a witness unit selected inside one unit, as the workbench receives it (display offsets). */
function passageOf({range,element}){
  const unitNode=element.closest('[data-unit][data-chapter]'),language=element.closest('[lang]')?.lang;
  if(!unitNode||!LANGUAGES.has(language)||!unitNode.contains(range.startContainer)||!unitNode.contains(range.endContainer))return null;
  return {language,chapter:unitNode.dataset.chapter,unitId:unitNode.dataset.unit,start:displayPosition(unitNode,range),end:displayPosition(unitNode,range,{end:true}),display:true};
}
const benchWord=(key,locale)=>{
  const ru={lay:'На верстак',laid:'Фрагмент лежит на верстаке',failed:'Это место не удалось положить на верстак'}[key];
  const en={lay:'To the workbench',laid:'The fragment lies on the workbench',failed:'This place could not be laid on the workbench'}[key];
  return copyLocale(locale)==='en'?localeText(en,locale):ru;
};

/**
 * Offer the origin of words selected inside `root` in elements carrying `data-chapter` and `data-unit`
 * (a witness unit) within an element whose `lang` names the witness. `openPlace(place)` opens a place of the book.
 * With the stand's `renderings` and `terms`, a word of a term also offers its renderings; `fragment(cite)` shows their places.
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
  const translationOf=value=>{const language=value.element.closest('[lang]')?.lang;return language==='de'?translationLanguage(locale()):language;};
  const termsOf=async value=>{
    const found=await word(value);if(!found)return [];
    return wordRenderings(renderings,terms,{language:found.language,form:value.text,lemma:found.context?.words?.[0]?.lemma});
  };
  async function showRenderings(value){
    const found=await word(value),entries=await termsOf(value);if(!found||!entries.length)return;
    const language=found.language==='de'?translationLanguage(locale()):found.language;
    const {body}=open();
    body.append(Object.assign(document.createElement('h2'),{className:'cr-inspector-title',textContent:renderingsWord('translations',locale())}));
    for(const entry of entries)renderRenderings(body,entry,{language,locale:locale(),fragment});
  }
  // A word or a whole passage goes onto the workbench, where the stand has one.
  const layAction={get label(){return benchWord('lay',locale());},phrases:true,when:value=>benchListens()&&Boolean(passageOf(value)),
    run:value=>{const passage=passageOf(value);return passage?layOnBench(passage).then(laid=>benchWord(laid?'laid':'failed',locale())):null;}};
  const menu=createSelectionMenu({container:root,accept:value=>Boolean(value.element.closest('[data-unit][data-chapter]')),
    actions:[{get label(){return label();},run:value=>void show(value)},
      ...(renderings&&fragment?[{get label(){return renderingsWord('translations',locale());},run:value=>void showRenderings(value),
        when:value=>termsOf(value).then(entries=>entries.some(entry=>entry[translationOf(value)]))}]:[]),layAction]});
  return {dispose(){menu.dispose();dialog?.remove();dialog=null;}};
}
