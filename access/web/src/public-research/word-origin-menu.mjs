// The stand's quotations and parallel reading offer the origin of a selected
// word of the book: a small menu beside the word, then the origin in a sheet.
import {createSelectionMenu} from '../corpus-reader/selection-menu.mjs';
import {renderOrigin} from '../corpus-reader/word-origin.mjs';
import {bonsaiWordByForm} from '../corpus-reader/bonsai-words.mjs';
import {copyLocale,localeText} from './locale.mjs';
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
 */
export function createStandOrigin({root,locale,openPlace=null}){
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
  async function show({text,range,element}){
    const unitNode=element.closest('[data-unit][data-chapter]'),langNode=element.closest('[lang]');
    const language=langNode?.lang;
    if(!unitNode||!LANGUAGES.has(language))return;
    const context=await bonsaiWordByForm({chapter:unitNode.dataset.chapter,language,unitId:unitNode.dataset.unit,form:text,
      near:displayPosition(unitNode,range)}).catch(()=>null);
    const view=sheet(),body=view.querySelector('.ps-origin-body');
    body.replaceChildren();
    if(!view.open)view.showModal();
    await renderOrigin(body,{locale:locale(),form:text,language,context,
      onOpenPlace:openPlace?place=>{view.close();openPlace(place);}:undefined});
  }
  const menu=createSelectionMenu({container:root,accept:value=>Boolean(value.element.closest('[data-unit][data-chapter]')),
    actions:[{get label(){return label();},run:value=>void show(value)}]});
  return {dispose(){menu.dispose();dialog?.remove();dialog=null;}};
}
