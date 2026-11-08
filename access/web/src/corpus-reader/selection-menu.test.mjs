import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
import {createSelectionMenu} from './selection-menu.mjs';

// The menu beside a selected word stands in the top layer, inside the open dialog holding the word.
class Node {
  constructor(tag,doc){this.tagName=tag.toUpperCase();this.ownerDocument=doc;this.children=[];this.parentElement=null;this.attributes={};this.listeners={};this.style={};this.hidden=false;this.textContent='';this.nodeType=1;this.className='';this.open=false;this.popoverOpen=false;}
  setAttribute(name,value){this.attributes[name]=String(value);if(name==='open')this.open=true;}
  hasAttribute(name){return name in this.attributes;}
  append(...nodes){for(const node of nodes){node.parentElement?.children.splice(node.parentElement.children.indexOf(node),1);node.parentElement=this;this.children.push(node);}}
  remove(){if(this.parentElement){this.parentElement.children.splice(this.parentElement.children.indexOf(this),1);this.parentElement=null;}}
  contains(node){for(let at=node;at;at=at.parentElement)if(at===this)return true;return false;}
  get isConnected(){for(let at=this;at;at=at.parentElement)if(at===this.ownerDocument.body)return true;return false;}
  closest(selector){
    for(let at=this;at;at=at.parentElement){
      if(selector==='dialog[open]'&&at.tagName==='DIALOG'&&at.open)return at;
      if(selector.startsWith('.')&&at.className.split(' ').includes(selector.slice(1)))return at;
    }
    return null;
  }
  matches(selector){return selector===':popover-open'?this.popoverOpen:false;}
  showPopover(){if(this.popoverOpen)throw new Error('already open');this.popoverOpen=true;this.ownerDocument.topLayer.push(this);}
  hidePopover(){this.popoverOpen=false;this.ownerDocument.topLayer.splice(this.ownerDocument.topLayer.indexOf(this),1);}
  addEventListener(name,fn){(this.listeners[name]??=[]).push(fn);}
  removeEventListener(name,fn){this.listeners[name]=(this.listeners[name]??[]).filter(item=>item!==fn);}
  click(){for(const fn of this.listeners.click??[])fn({preventDefault(){}});}
  get offsetWidth(){return 150;}
  get offsetHeight(){return 30;}
}
function page(){
  const doc={listeners:{},topLayer:[],addEventListener(name,fn){(this.listeners[name]??=[]).push(fn);},removeEventListener(){},createElement:tag=>new Node(tag,doc)};
  doc.body=new Node('body',doc);
  let selection=null;
  doc.defaultView={innerWidth:1200,setTimeout:(fn,ms)=>setTimeout(fn,ms),clearTimeout:id=>clearTimeout(id),addEventListener(){},removeEventListener(){},getSelection:()=>selection};
  const select=(element,text)=>{
    const textNode={nodeType:3,parentElement:element};
    selection=text?{rangeCount:1,isCollapsed:false,toString:()=>text,getRangeAt:()=>({commonAncestorContainer:textNode,getBoundingClientRect:()=>({top:200,bottom:220,left:300,width:60})})}:null;
    for(const fn of doc.listeners.selectionchange??[])fn();
    vi.advanceTimersByTime(150);
  };
  return {doc,select};
}
const shown=menu=>!menu.hidden&&menu.popoverOpen;
const labels=menu=>menu.children.filter(button=>!button.hidden).map(button=>button.textContent);

describe('the menu beside a selected word',()=>{
  beforeEach(()=>vi.useFakeTimers());
  afterEach(()=>vi.useRealTimers());

  it('stands in the top layer inside the open dialog that holds the word, and returns to the page after',()=>{
    const {doc,select}=page();
    const dialog=doc.createElement('dialog');dialog.setAttribute('open','');doc.body.append(dialog);
    const line=doc.createElement('p');line.className='line';dialog.append(line);
    const outside=doc.createElement('p');outside.className='line';doc.body.append(outside);
    const runs=[];
    const {element:menu}=createSelectionMenu({container:doc.body,ownerDocument:doc,accept:value=>Boolean(value.element.closest('.line')),
      actions:[{label:'Origin',run:value=>runs.push(value.text)}]});
    select(line,'Übermenschen');
    expect(menu.parentElement).toBe(dialog);
    expect(shown(menu)).toBe(true);
    expect(doc.topLayer).toEqual([menu]);
    expect(menu.style).toEqual({top:'162px',left:'255px'});
    menu.children[0].click();
    expect(runs).toEqual(['Übermenschen']);
    expect(shown(menu)).toBe(false);
    dialog.open=false;
    select(outside,'Erde');
    expect(menu.parentElement).toBe(doc.body);
    expect(shown(menu)).toBe(true);
  });

  it('shows each action once it says yes for that same selection, and nothing for a phrase or a refused place',async()=>{
    const {doc,select}=page();
    const line=doc.createElement('p');line.className='line';doc.body.append(line);
    const other=doc.createElement('p');doc.body.append(other);
    let answer;
    const {element:menu,setActions}=createSelectionMenu({container:doc.body,ownerDocument:doc,accept:value=>Boolean(value.element.closest('.line')),
      actions:[{label:'Origin',when:()=>new Promise(resolve=>{answer=resolve;}),run(){}},{label:'Never',when:()=>false,run(){}}]});
    select(line,'Erde');
    expect(shown(menu)).toBe(false);
    answer(true);await Promise.resolve();await Promise.resolve();
    expect(shown(menu)).toBe(true);
    expect(labels(menu)).toEqual(['Origin']);
    select(line,'die Erde');
    expect(shown(menu)).toBe(false);
    select(other,'Erde');
    expect(shown(menu)).toBe(false);
    // A late yes for a selection that has gone shows nothing.
    select(line,'Sinn');const late=answer;select(line,'');late(true);await Promise.resolve();await Promise.resolve();
    expect(shown(menu)).toBe(false);
    setActions([{label:'Origin',run(){}},{label:'Translations',when:()=>Promise.resolve(true),run(){}}]);
    select(line,'Erde');await Promise.resolve();await Promise.resolve();
    expect(labels(menu)).toEqual(['Origin','Translations']);
  });
});
