// A small menu beside a selected word. It appears when a single word is
// selected inside its container, asks for nothing, loads nothing, and leaves
// when the selection goes, on Escape or on scroll. Its actions are given by the
// caller: the reader offers the word's origin; an action may hold itself back
// for a word it has nothing for, deciding at once or once it knows. An action
// marked `phrases` also stands beside a longer passage, alone with its kind.
import './selection-menu.css';

const SINGLE_WORD=/^[\p{L}\p{M}][\p{L}\p{M}'’¬\-]*$/u;

/** Whether a selected text is one word (a word broken at a line end counts as one). */
export function isSingleWord(text){
  const value=String(text??'').replace(/¬\s*/gu,'').trim();
  return value.length>0&&value.length<=48&&SINGLE_WORD.test(value);
}

const PASSAGE_LIMIT=2000;

/**
 * Show `actions` ([{label, run(selection), when?(selection), phrases?}]) beside a single-word selection inside `container`;
 * an action with `phrases: true` also shows beside a passage of several words (the selection then has `single: false`).
 * `accept(selection)` may refuse a selection (return false); `when(selection)` may keep an action back for it,
 * returning false or a promise of false. Returns {element, hide(), setActions(actions), dispose()}.
 *
 * The menu stands in the top layer, in the open dialog that holds the selection when there is one: above the
 * parallel reading or a sheet, and reachable while that dialog leaves the rest of the page inert.
 */
export function createSelectionMenu({container,actions,accept=()=>true,ownerDocument=document}){
  const view=ownerDocument.defaultView;
  const menu=ownerDocument.createElement('div');
  menu.className='tos-selection-menu';
  menu.setAttribute('role','toolbar');
  const popover=typeof menu.showPopover==='function';
  if(popover)menu.setAttribute('popover','manual');
  menu.hidden=true;
  let current=null,timer=0,list=[],buttons=[];
  const note=ownerDocument.createElement('div');
  note.className='tos-selection-note';note.setAttribute('role','status');note.hidden=true;
  let noteTimer=0;
  const open=()=>popover&&menu.isConnected&&menu.matches(':popover-open');
  function setActions(next){
    list=[...(next??[])];
    for(const button of buttons)button.remove();
    buttons=list.map(action=>{
      const button=ownerDocument.createElement('button');
      button.type='button';button.className='tos-selection-menu-action';button.textContent=action.label;
      // Keep the selection: a pointer press on the menu does not collapse it.
      button.addEventListener('pointerdown',event=>event.preventDefault());
      button.addEventListener('mousedown',event=>event.preventDefault());
      button.addEventListener('click',()=>{
        const value=current,parent=menu.parentElement,at=value?.range.getBoundingClientRect();hide();
        if(value)void Promise.resolve(action.run(value)).then(text=>{
          if(typeof text!=='string'||!text||!menu.isConnected)return;
          parent.append(note);note.textContent=text;note.hidden=false;
          note.style.top=`${at.top}px`;note.style.left=`${Math.max(8,Math.min(at.left,view.innerWidth-note.offsetWidth-8))}px`;
          view.clearTimeout(noteTimer);noteTimer=view.setTimeout(()=>{note.hidden=true;},2200);
        },()=>{});
      });
      menu.append(button);
      return button;
    });
    hide();
  }
  setActions(actions);
  ownerDocument.body.append(menu);
  function hide(){current=null;menu.hidden=true;if(open())menu.hidePopover();}
  function show(element){
    const layer=element.closest?.('dialog[open]')??ownerDocument.body;
    if(menu.parentElement!==layer){if(open())menu.hidePopover();layer.append(menu);}
    menu.hidden=false;
    // Shown again each time, so it stands above a dialog opened since.
    if(popover){if(open())menu.hidePopover();menu.showPopover();}
  }
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
    const single=isSingleWord(text);
    if(!element||!container.contains(element)||!single&&!(list.some(action=>action.phrases)&&text.trim().length<=PASSAGE_LIMIT)){hide();return;}
    const value={text:text.replace(/¬\s*/gu,'').trim(),range,element,single};
    if(accept(value)===false){hide();return;}
    hide();
    current=value;
    // An action that is still deciding joins the menu when it says yes for this same selection.
    const reveal=button=>{if(current!==value)return;button.hidden=false;show(element);place(range.getBoundingClientRect());};
    list.forEach((action,index)=>{
      const button=buttons[index];
      button.textContent=action.label;button.hidden=true;
      if(!single&&!action.phrases)return;
      const answer=action.when?action.when(value):true;
      if(answer===true)button.hidden=false;
      else if(typeof answer?.then==='function')answer.then(yes=>{if(yes)reveal(button);},()=>{});
    });
    if(buttons.some(button=>!button.hidden)){show(element);place(range.getBoundingClientRect());}
  }
  const onSelection=()=>{view.clearTimeout(timer);timer=view.setTimeout(update,120);};
  const onKey=event=>{if(event.key==='Escape'&&!menu.hidden){event.preventDefault();hide();}};
  const onScroll=()=>{if(!menu.hidden&&current)place(current.range.getBoundingClientRect());};
  ownerDocument.addEventListener('selectionchange',onSelection);
  ownerDocument.addEventListener('keydown',onKey);
  view.addEventListener('scroll',onScroll,true);
  return {
    element:menu,hide,setActions,
    dispose(){view.clearTimeout(timer);view.clearTimeout(noteTimer);note.remove();ownerDocument.removeEventListener('selectionchange',onSelection);ownerDocument.removeEventListener('keydown',onKey);
      view.removeEventListener('scroll',onScroll,true);hide();menu.remove();},
  };
}
