// The controls of the space, shown once the way a game shows its controls:
// gestures only, because a gesture has nothing on the screen to show it, and
// never a list of buttons. "Don't show again" is remembered in this browser;
// closing without it shows the window again on the next visit.

export const CONTROLS_KEY='tos.public-research.controls.v1';
const HIDDEN='hidden';

/** Whether the reader has asked not to see the window again. Unreadable storage reads as "not asked". */
export function controlsDismissed(storage){
  try{return storage?.getItem?.(CONTROLS_KEY)===HIDDEN;}catch{return false;}
}

/** Remember (or forget) "don't show again"; reports whether the browser kept it. */
export function rememberControls(storage,hidden=true){
  try{
    if(hidden)storage.setItem(CONTROLS_KEY,HIDDEN);else storage.removeItem(CONTROLS_KEY);
    return storage.getItem(CONTROLS_KEY)===(hidden?HIDDEN:null);
  }catch{return false;}
}

/**
 * Whether the window opens by itself now: on a space that turns, after its
 * opening turn, and only where the browser can remember a "don't show again"
 * — a window that could never be dismissed for good does not insist.
 */
export function controlsShouldOpen({storage,flat=false,opened=false}={}){
  if(flat||opened)return false;
  try{const probe=`${CONTROLS_KEY}.probe`;storage.setItem(probe,'1');storage.removeItem(probe);}catch{return false;}
  return !controlsDismissed(storage);
}

const WORDS={
  ru:{
    title:'Управление',close:'Понятно',never:'Больше не показывать',reopen:'Управление пространством',
    turn:'повернуть пространство',move:'сдвинуть',zoom:'приблизить и отдалить',lift:'поднять звезду и переставить',
    keys:'повернуть пространство',route:'на маршруте — шаги по нему, а с Shift — поворот',choose:'открыть звезду или связь',
    drag:'Тянуть',shiftDrag:'Shift + тянуть',rightDrag:'Правой кнопкой',wheel:'Колесо',hold:'Задержать на звезде',arrows:'Стрелки',click:'Нажать',
    oneFinger:'Одним пальцем',twoFingers:'Двумя пальцами',pinch:'Свести или развести пальцы',holdFinger:'Задержать палец на звезде',tap:'Коснуться',
    or:'или',
  },
  en:{
    title:'Controls',close:'Got it',never:'Don’t show again',reopen:'Controls of the space',
    turn:'turn the space',move:'move it',zoom:'zoom in and out',lift:'pick up a star and move it',
    keys:'turn the space',route:'on a route they walk its steps, with Shift they turn',choose:'open a star or a link',
    drag:'Drag',shiftDrag:'Shift + drag',rightDrag:'Right button',wheel:'Wheel',hold:'Hold on a star',arrows:'Arrows',click:'Click',
    oneFinger:'One finger',twoFingers:'Two fingers',pinch:'Pinch',holdFinger:'Hold a finger on a star',tap:'Tap',
    or:'or',
  },
};
export const controlsWords=locale=>WORDS[String(locale||'ru').startsWith('en')?'en':'ru'];

/**
 * The gestures of a space read in depth, for a pointer that hovers (mouse and
 * keyboard) or for fingers. Each row is a gesture, a sign drawn for it and what it does.
 */
export function controlsFor(device='mouse',locale='ru'){
  const w=controlsWords(locale);
  if(device==='touch')return [
    {sign:'finger',gesture:w.oneFinger,action:w.turn},
    {sign:'fingers',gesture:w.twoFingers,action:w.move},
    {sign:'pinch',gesture:w.pinch,action:w.zoom},
    {sign:'hold',gesture:w.holdFinger,action:w.lift},
    {sign:'tap',gesture:w.tap,action:w.choose},
  ];
  return [
    {sign:'drag',gesture:w.drag,action:w.turn},
    {sign:'pan',gesture:`${w.shiftDrag} ${w.or} ${w.rightDrag.toLocaleLowerCase()}`,action:w.move},
    {sign:'wheel',gesture:w.wheel,action:w.zoom},
    {sign:'hold',gesture:w.hold,action:w.lift},
    {sign:'arrows',gesture:w.arrows,action:`${w.keys}; ${w.route}`},
    {sign:'click',gesture:w.click,action:w.choose},
  ];
}

const SVG='http://www.w3.org/2000/svg';
// Small drawn signs for the gestures: a mouse and its buttons, a finger, the arrows.
function sign(kind){
  const svg=document.createElementNS(SVG,'svg');svg.setAttribute('viewBox','0 0 32 32');svg.setAttribute('aria-hidden','true');
  const add=(tag,attrs)=>{const node=document.createElementNS(SVG,tag);for(const [key,value]of Object.entries(attrs))node.setAttribute(key,value);svg.append(node);return node;};
  const line={fill:'none',stroke:'currentColor','stroke-width':'1.4','stroke-linecap':'round','stroke-linejoin':'round'};
  const mouse=(button)=>{add('rect',{x:'10',y:'6',width:'12',height:'19',rx:'6',...line});add('path',{d:'M16 6v7',...line});if(button==='left')add('path',{d:'M16 6.6a5.6 5.6 0 0 0-5.4 5.4v1h5.4z',fill:'currentColor',opacity:'.55'});if(button==='right')add('path',{d:'M16 6.6a5.6 5.6 0 0 1 5.4 5.4v1H16z',fill:'currentColor',opacity:'.55'});if(button==='wheel')add('path',{d:'M16 9v2.6',stroke:'currentColor','stroke-width':'2.4','stroke-linecap':'round'});};
  const finger=(x,y)=>add('circle',{cx:String(x),cy:String(y),r:'3.2',...line});
  switch(kind){
    case 'drag':mouse('left');add('path',{d:'M4 27c3-2.4 5.4-2.4 8 0M24 27c-3-2.4-5.4-2.4-8 0',...line,opacity:'.7'});break;
    case 'pan':mouse('right');add('path',{d:'M4 16h3M25 16h3M5.6 14.4 4 16l1.6 1.6M26.4 14.4 28 16l-1.6 1.6',...line,opacity:'.7'});break;
    case 'wheel':mouse('wheel');add('path',{d:'M27 9v8M25.4 10.6 27 9l1.6 1.6M25.4 15.4 27 17l1.6-1.6',...line,opacity:'.7'});break;
    case 'hold':add('circle',{cx:'16',cy:'16',r:'4.2',...line});add('circle',{cx:'16',cy:'16',r:'9',...line,'stroke-dasharray':'2.4 3',opacity:'.7'});break;
    case 'arrows':for(const [x,y,d] of [[12,18,'M14.8 22.6 12 20.6l2.8-2'],[16,14,'M13.6 16.8 16 14l2.4 2.8'],[20,18,'M17.2 22.6l2.8-2-2.8-2']]){add('rect',{x:String(x-2.6),y:String(y+0.4),width:'7.2',height:'6',rx:'1.4',...line,opacity:'.75'});add('path',{d,...line});}break;
    case 'click':mouse('left');break;
    case 'finger':finger(16,14);add('path',{d:'M8 24c4-3 12-3 16 0M21 21.4l3 2.6-3.6 1',...line,opacity:'.7'});break;
    case 'fingers':finger(12,13);finger(20,13);add('path',{d:'M8 24h16M21 21.4 24 24l-3 2.6',...line,opacity:'.7'});break;
    case 'pinch':finger(10,20);finger(22,12);add('path',{d:'M13 17.4 19.4 14.6',...line,opacity:'.7'});break;
    case 'tap':finger(16,16);add('circle',{cx:'16',cy:'16',r:'7.4',...line,opacity:'.45'});break;
    default:break;
  }
  return svg;
}

const make=(tag,className,text)=>{const node=document.createElement(tag);if(className)node.className=className;if(text!==undefined)node.textContent=text;return node;};

/**
 * Whether Escape belongs to the window: not while a modal dialog, the reader
 * or anything else holds the page above it, and not while a word is composed.
 */
export function escapeReaches(panel,event){
  if(event.isComposing||panel.hidden)return false;
  if(panel.closest('[inert]'))return false;
  try{if(document.querySelector('dialog:modal'))return false;}catch{}
  const active=document.activeElement;
  return !active||active===document.body||active===document.documentElement||panel.parentElement?.contains(active);
}

/**
 * Mount the window on `root`, right after `anchor` (the map tools) so the
 * keyboard reaches it next to them. It opens with `show()` and closes by its
 * button or Escape. Ticking "don't show again" is kept at once, and only a
 * reader's own untick forgets it: closing never rewrites the choice, so a
 * second tab whose window opened earlier cannot undo it. `device()` says
 * whether the reader points with a mouse or with fingers, `locale()` which
 * words to use.
 */
export function mountControls(root,{storage=globalThis.localStorage,locale=()=> 'ru',device=()=> 'mouse',anchor=null,onClose=null}={}){
  const panel=make('section','ps-controls');panel.hidden=true;panel.setAttribute('role','dialog');panel.setAttribute('aria-modal','false');
  const heading=make('h2','ps-controls-title'),list=make('ul','ps-controls-list'),foot=make('div','ps-controls-foot');
  const neverLabel=make('label','ps-controls-never'),never=make('input');never.type='checkbox';const neverText=make('span');neverLabel.append(never,neverText);
  const close=make('button','ps-controls-close');close.type='button';
  heading.id=`ps-controls-title-${Math.random().toString(36).slice(2,8)}`;panel.setAttribute('aria-labelledby',heading.id);
  foot.append(neverLabel,close);panel.append(heading,list,foot);
  if(anchor?.parentElement===root)anchor.after(panel);else root.append(panel);
  never.addEventListener('change',()=>{rememberControls(storage,never.checked);});
  // Another tab may have chosen meanwhile; the box shows what is kept when this one comes back.
  const sync=()=>{if(!document.hidden&&!panel.hidden)never.checked=controlsDismissed(storage);};
  document.addEventListener('visibilitychange',sync);
  let returnTo=null;
  function paint(){
    const w=controlsWords(locale());heading.textContent=w.title;neverText.textContent=w.never;close.textContent=w.close;
    list.replaceChildren(...controlsFor(device(),locale()).map(row=>{const item=make('li','ps-controls-row');item.dataset.sign=row.sign;const glyph=make('span','ps-controls-sign');glyph.append(sign(row.sign));item.append(glyph,make('span','ps-controls-gesture',row.gesture),make('span','ps-controls-action',row.action));return item;}));
  }
  // Closing only closes: what the next visit does was kept when the box changed.
  function finish(){
    if(panel.hidden)return;
    panel.hidden=true;root.dataset.controls='closed';
    if(returnTo?.isConnected&&panel.contains(document.activeElement))returnTo.focus({preventScroll:true});
    returnTo=null;onClose?.({never:controlsDismissed(storage)});
  }
  close.addEventListener('click',finish);
  const key=event=>{if(event.key==='Escape'&&escapeReaches(panel,event)){event.preventDefault();event.stopImmediatePropagation();finish();}};
  document.addEventListener('keydown',key,{capture:true});
  return {
    get element(){return panel;},
    get open(){return !panel.hidden;},
    /** Open the window; `focus` moves the keyboard into it (when opened on request). */
    show({focus=false}={}){
      paint();never.checked=controlsDismissed(storage);panel.hidden=false;root.dataset.controls='open';
      if(focus){returnTo=document.activeElement instanceof HTMLElement?document.activeElement:null;close.focus({preventScroll:true});}
    },
    close:finish,
    refresh(){if(!panel.hidden)paint();},
    dispose(){document.removeEventListener('keydown',key,{capture:true});document.removeEventListener('visibilitychange',sync);panel.remove();},
  };
}
