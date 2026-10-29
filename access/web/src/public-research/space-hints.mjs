// One hint for a whole stand. It answers a pointer that rests on something, or
// a keyboard that reaches it, next to the thing itself; it never stays on the
// screen by itself and never repeats what is already written on the thing.
//
// An element gives a hint by its `data-hint` (a line of text) and, where a key
// does the same, `data-hint-keys`; a stand may describe richer things itself,
// such as a star's card, through `describe`.

const KEY_SEPARATOR='\u0001';
let mounted=0;

/** Mark an element with its hint: a line of text and the keys that do the same, if any. */
export function setHint(element,text,keys=null,{note=''}={}){
  if(!element)return element;
  element.removeAttribute('title');
  if(text)element.dataset.hint=text;else delete element.dataset.hint;
  const list=(Array.isArray(keys)?keys:keys?[keys]:[]).filter(Boolean);
  if(list.length)element.dataset.hintKeys=list.join(KEY_SEPARATOR);else delete element.dataset.hintKeys;
  if(note)element.dataset.hintNote=note;else delete element.dataset.hintNote;
  return element;
}

/**
 * What a marked element says, or null. A hint that only repeats the text the
 * element already shows says nothing, unless it also names a key.
 */
export function hintWords({hint='',keys='',note='',shown=''}={}){
  const text=String(hint??'').replace(/\s+/g,' ').trim();if(!text)return null;
  const list=String(keys??'').split(KEY_SEPARATOR).map(key=>key.trim()).filter(Boolean);
  const visible=String(shown??'').replace(/\s+/g,' ').trim().toLocaleLowerCase();
  if(!list.length&&!note&&visible&&visible.includes(text.toLocaleLowerCase()))return null;
  return {text,keys:list,note:String(note??'').trim()};
}

/**
 * Where a hint of `size` stands beside a thing at `anchor`, inside `viewport`:
 * on the preferred side if it fits there, else on the first side that does,
 * and always inside the view.
 */
export function placeHint(anchor,size,viewport,side='top',{gap=8,margin=8}={}){
  const {width,height}=size,{width:vw,height:vh}=viewport;
  const middleX=anchor.left+(anchor.right-anchor.left)/2-width/2,middleY=anchor.top+(anchor.bottom-anchor.top)/2-height/2;
  const at={top:[middleX,anchor.top-height-gap],bottom:[middleX,anchor.bottom+gap],right:[anchor.right+gap,middleY],left:[anchor.left-width-gap,middleY]};
  const orders={top:['top','bottom','right','left'],bottom:['bottom','top','right','left'],right:['right','left','top','bottom'],left:['left','right','top','bottom']};
  const fits=([x,y])=>x>=margin&&y>=margin&&x+width<=vw-margin&&y+height<=vh-margin;
  const order=orders[side]??orders.top,chosen=order.find(name=>fits(at[name]))??order[0];
  const clamp=(value,low,high)=>Math.min(Math.max(value,low),Math.max(low,high));
  const [x,y]=at[chosen];
  return {x:clamp(x,margin,vw-margin-width),y:clamp(y,margin,vh-margin-height),side:chosen};
}

const make=(tag,className,text)=>{const node=document.createElement(tag);if(className)node.className=className;if(text!==undefined)node.textContent=text;return node;};

/** The text an element really shows: words clipped to nothing, hidden or transparent are not shown. */
function shownText(element){
  const parts=[],walker=document.createTreeWalker(element,NodeFilter.SHOW_TEXT);
  for(let node=walker.nextNode();node;node=walker.nextNode()){
    const text=node.textContent.trim();if(!text)continue;
    const style=node.parentElement?getComputedStyle(node.parentElement):null;
    if(style&&(style.visibility==='hidden'||style.display==='none'||Number(style.opacity)===0))continue;
    const range=document.createRange();range.selectNodeContents(node);const box=range.getBoundingClientRect();
    if(box.width<3||box.height<3)continue;
    // A word cut away by a clipping box is not shown either.
    const clip=node.parentElement?.closest('*');const own=clip?.getBoundingClientRect();if(own&&(own.width<3||own.height<3))continue;
    parts.push(text);
  }
  return parts.join(' ');
}

/**
 * Mount the hint on `root`. `selector` names what can give a hint;
 * `describe(element)` may return a card ({eyebrow,title,lines,note}) or words
 * ({text,keys,note}) for an element, and otherwise its data attributes speak.
 */
export function mountHints(root,{selector='[data-hint]',describe=null,delay=380,side=()=> 'top',moving=()=>false}={}){
  const tip=make('div','ps-hint');tip.id=`ps-hint-${++mounted}`;tip.setAttribute('role','tooltip');tip.hidden=true;root.append(tip);
  let target=null,pending=null,timer=0,warmUntil=0,follow=0,disposed=false;
  const find=node=>node instanceof Element?node.closest(selector):null;
  const words=element=>{
    const described=describe?.(element);if(described!==undefined)return described;
    return hintWords({hint:element.dataset.hint,keys:element.dataset.hintKeys,note:element.dataset.hintNote,shown:shownText(element)});
  };
  function render(content){
    tip.replaceChildren();
    if(content.title||content.lines||content.eyebrow){
      tip.dataset.form='card';
      if(content.eyebrow)tip.append(make('span','ps-hint-eyebrow',content.eyebrow));
      if(content.title)tip.append(make('strong','ps-hint-title',content.title));
      for(const line of content.lines??[])if(line)tip.append(make('span','ps-hint-line',line));
    }else{
      tip.dataset.form='words';
      const row=make('span','ps-hint-row');row.append(make('span','ps-hint-text',content.text));
      if(content.keys?.length){const keys=make('span','ps-hint-keys');for(const key of content.keys)keys.append(make('kbd','',key));row.append(keys);}
      tip.append(row);
    }
    if(content.note)tip.append(make('span','ps-hint-note',content.note));
  }
  // A star is pointed at with its name beside it, so the hint keeps clear of both.
  function anchorOf(element){
    const own=element.getBoundingClientRect(),box={left:own.left,top:own.top,right:own.right,bottom:own.bottom};
    for(const part of element.querySelectorAll(':scope > span')){
      if(getComputedStyle(part).opacity==='0')continue;const r=part.getBoundingClientRect();if(!r.width||!r.height)continue;
      box.left=Math.min(box.left,r.left);box.top=Math.min(box.top,r.top);box.right=Math.max(box.right,r.right);box.bottom=Math.max(box.bottom,r.bottom);
    }
    return box;
  }
  function position(){
    if(!target)return;
    if(!target.isConnected||target.hidden||target.closest('[hidden]')){hide();return;}
    const placed=placeHint(anchorOf(target),{width:tip.offsetWidth,height:tip.offsetHeight},{width:innerWidth,height:innerHeight},side(target));
    tip.dataset.side=placed.side;tip.style.transform=`translate3d(${Math.round(placed.x)}px,${Math.round(placed.y)}px,0)`;
  }
  // Things that move with the space keep their hint beside them.
  function track(){cancelAnimationFrame(follow);follow=0;if(!target||!moving(target))return;const step=()=>{if(!target)return;position();follow=requestAnimationFrame(step);};follow=requestAnimationFrame(step);}
  function show(element){
    clearTimeout(timer);pending=null;
    const content=element.isConnected?words(element):null;
    if(!content){hide();return;}
    if(target&&target!==element)target.removeAttribute('aria-describedby');
    target=element;render(content);tip.hidden=false;position();track();
    element.setAttribute('aria-describedby',tip.id);
  }
  function schedule(element,wait){clearTimeout(timer);pending=element;timer=setTimeout(()=>{if(pending===element)show(element);},Math.max(0,wait));}
  function hide(){
    clearTimeout(timer);pending=null;cancelAnimationFrame(follow);follow=0;
    if(target){target.removeAttribute('aria-describedby');warmUntil=performance.now()+700;}
    target=null;tip.hidden=true;
  }
  // A hint that is already open hands over at once to the next thing the pointer rests on.
  const over=event=>{
    if(event.pointerType==='touch')return;
    const element=find(event.target);if(!element||element===target||element===pending)return;
    schedule(element,target||performance.now()<warmUntil?60:delay);
  };
  const out=event=>{
    const element=find(event.target);if(!element||(element!==target&&element!==pending))return;
    if(event.relatedTarget instanceof Node&&element.contains(event.relatedTarget))return;
    hide();
  };
  // The keyboard gets the hint as soon as it arrives; a click's focus does not.
  const focusIn=event=>{const element=find(event.target);if(element&&event.target.matches?.(':focus-visible'))schedule(element,120);};
  // A hint the keyboard has already left neither stays nor arrives late.
  const focusOut=event=>{
    const into=event.relatedTarget instanceof Node?event.relatedTarget:null;
    if(pending&&!(into&&pending.contains(into))){clearTimeout(timer);pending=null;}
    if(target&&!(into&&target.contains(into)))hide();
  };
  // Escape closes the hint first, and only the hint; while a word is composed, or a dialog holds the page, it is not ours.
  const key=event=>{
    if(event.key!=='Escape'||tip.hidden||event.isComposing)return;
    let modal=null;try{modal=document.querySelector('dialog:modal');}catch{}
    if(modal&&!modal.contains(target)){hide();return;}
    event.preventDefault();event.stopImmediatePropagation();hide();
  };
  const press=()=>hide();
  root.addEventListener('pointerover',over);root.addEventListener('pointerout',out);
  root.addEventListener('focusin',focusIn);root.addEventListener('focusout',focusOut);
  root.addEventListener('pointerdown',press,{capture:true});root.addEventListener('wheel',press,{passive:true,capture:true});
  document.addEventListener('keydown',key,{capture:true});window.addEventListener('scroll',press,{capture:true,passive:true});
  return {
    /** Give the hint of an element now, as if the pointer had rested on it (a link's line, a scripted focus). */
    showFor(element,{wait=delay}={}){if(disposed||!element)return;if(element===target)return;schedule(element,target||performance.now()<warmUntil?60:wait);},
    /** Close the hint if it belongs to `element`, or whichever is open. */
    hide(element=null){if(!element||element===target||element===pending)hide();},
    get element(){return tip;},
    get target(){return target;},
    dispose(){
      disposed=true;hide();tip.remove();
      root.removeEventListener('pointerover',over);root.removeEventListener('pointerout',out);
      root.removeEventListener('focusin',focusIn);root.removeEventListener('focusout',focusOut);
      root.removeEventListener('pointerdown',press,{capture:true});root.removeEventListener('wheel',press,{capture:true});
      document.removeEventListener('keydown',key,{capture:true});window.removeEventListener('scroll',press,{capture:true});
    },
  };
}
