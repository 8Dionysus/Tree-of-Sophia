// A small menu beside a selected word. It appears when a single word is
// selected inside its container, asks for nothing, loads nothing, and leaves
// when the selection goes, on Escape or on scroll. Its actions are given by the
// caller: the reader offers the word's origin; an action may hold itself back
// for a word it has nothing for, deciding at once or once it knows.
import './selection-menu.css';

const SINGLE_WORD=/^[\p{L}\p{M}][\p{L}\p{M}'’¬\-]*$/u;

/** Whether a selected text is one word (a word broken at a line end counts as one). */
export function isSingleWord(text){
  const value=String(text??'').replace(/¬\s*/gu,'').trim();
  return value.length>0&&value.length<=48&&SINGLE_WORD.test(value);
}

/**
 * Show `actions` ([{label, run(selection), when?(selection)}]) beside a single-word selection inside `container`.
 * `accept(selection)` may refuse a selection (return false); `when(selection)` may keep an action back for it,
 * returning false or a promise of false. Returns {element, hide(), dispose()}.
 */
export function createSelectionMenu({container,actions,accept=()=>true,ownerDocument=document}){
  const view=ownerDocument.defaultView;
  const menu=ownerDocument.createElement('div');
  menu.className='tos-selection-menu';
  menu.setAttribute('role','toolbar');
  menu.hidden=true;
  let current=null,timer=0;
  const buttons=actions.map(action=>{
    const button=ownerDocument.createElement('button');
    button.type='button';button.className='tos-selection-menu-action';button.textContent=action.label;
    // Keep the selection: a pointer press on the menu does not collapse it.
    button.addEventListener('pointerdown',event=>event.preventDefault());
    button.addEventListener('mousedown',event=>event.preventDefault());
    button.addEventListener('click',()=>{const value=current;hide();if(value)action.run(value);});
    menu.append(button);
    return button;
  });
  ownerDocument.body.append(menu);
  function hide(){menu.hidden=true;current=null;}
  function place(rect){
    const width=menu.offsetWidth||160,height=menu.offsetHeight||34,margin=8;
    let top=rect.top-height-margin;if(top<margin)top=rect.bottom+margin;
    let left=rect.left+rect.width/2-width/2;left=Math.max(margin,Math.min(left,view.innerWidth-width-margin));
    menu.style.top=`${Math.round(top)}px`;menu.style.left=`${Math.round(left)}px`;
  }
  function update(){
    const selection=view.getSelection?.();
    if(!selection||selection.rangeCount===0||selection.isCollapsed){hide();return;}
    const range=selection.getRangeAt(0),text=selection.toString();
    const node=range.commonAncestorContainer,element=node.nodeType===1?node:node.parentElement;
    if(!element||!container.contains(element)||!isSingleWord(text)){hide();return;}
    const value={text:text.replace(/¬\s*/gu,'').trim(),range,element};
    if(accept(value)===false){hide();return;}
    current=value;
    // An action that is still deciding joins the menu when it says yes for this same selection.
    const reveal=button=>{if(current!==value)return;button.hidden=false;menu.hidden=false;place(range.getBoundingClientRect());};
    actions.forEach((action,index)=>{
      const button=buttons[index],answer=action.when?action.when(value):true;
      button.hidden=true;
      if(answer===true)button.hidden=false;
      else if(typeof answer?.then==='function')answer.then(yes=>{if(yes)reveal(button);},()=>{});
    });
    menu.hidden=buttons.every(button=>button.hidden);
    if(!menu.hidden)place(range.getBoundingClientRect());
  }
  const onSelection=()=>{view.clearTimeout(timer);timer=view.setTimeout(update,120);};
  const onKey=event=>{if(event.key==='Escape'&&!menu.hidden){hide();}};
  const onScroll=()=>{if(!menu.hidden&&current)place(current.range.getBoundingClientRect());};
  ownerDocument.addEventListener('selectionchange',onSelection);
  ownerDocument.addEventListener('keydown',onKey);
  view.addEventListener('scroll',onScroll,true);
  return {
    element:menu,hide,
    dispose(){view.clearTimeout(timer);ownerDocument.removeEventListener('selectionchange',onSelection);ownerDocument.removeEventListener('keydown',onKey);
      view.removeEventListener('scroll',onScroll,true);menu.remove();},
  };
}
