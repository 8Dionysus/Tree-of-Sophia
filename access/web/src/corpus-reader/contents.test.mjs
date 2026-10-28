import {afterEach, expect, it, vi} from 'vitest';
import {mountCorpusContents} from './contents.mjs';

// Minimal DOM for the edition-selection and navigation contract. Layout and
// focus appearance are verified separately in the real browser.
class Element {
  constructor(tag){this.tagName=tag;this.children=[];this.parent=null;this.attributes={};this.style={};this.events={};this.className='';this._text='';}
  set textContent(value){this._text=String(value);this.children=[];}
  get textContent(){return this._text+this.children.map(item=>item.textContent).join('');}
  append(...items){for(const item of items){item.parent=this;this.children.push(item);}}
  replaceChildren(...items){for(const child of this.children)child.parent=null;this.children=[];this.append(...items);}
  setAttribute(name,value){this.attributes[name]=String(value);}
  addEventListener(name,handler){this.events[name]=handler;}
  dispatch(name){this.events[name]?.({target:this});}
  click(){this.onclick?.({currentTarget:this});this.events.click?.({currentTarget:this});}
  focus(){document.activeElement=this;}
  remove(){if(this.parent)this.parent.children=this.parent.children.filter(item=>item!==this);this.parent=null;}
  get lastChild(){return this.children.at(-1);}
  querySelector(selector){
    const parts=selector.split(' '), match=(node,part)=>part.startsWith('.')?node.className.split(/\s+/).includes(part.slice(1)):node.tagName===part;
    const find=(node,index)=>{for(const child of node.children){if(match(child,parts[index])){if(index===parts.length-1)return child;const nested=find(child,index+1);if(nested)return nested;}const deeper=find(child,index);if(deeper)return deeper;}return null;};
    return find(this,0);
  }
}
afterEach(()=>vi.unstubAllGlobals());

it('chooses a comparison edition and ignores a late response from the other edition',async()=>{
  vi.stubGlobal('document',{activeElement:null,createElement:tag=>new Element(tag)});
  vi.stubGlobal('MutationObserver',class {observe(){} disconnect(){}});
  const root=new Element('section'),toolbar=new Element('nav');toolbar.className='cr-toolbar';root.append(toolbar);
  let finishGerman;
  const provider={capabilities:{structure:true},structure:vi.fn(({documentId,versionId})=>versionId==='de'
    ? new Promise(resolve=>{finishGerman=()=>resolve({documentId,versionId,revision:'de-rev',items:[{label:'German section',unitId:'de-unit'}]});})
    : Promise.resolve({documentId,versionId,revision:'en-rev',items:[{label:'English section',unitId:'en-unit'}]}))};
  const reader={element:root,state:()=>({document:{id:'work'},activeVersionId:'de'}),
    visibleVersions:()=>[{id:'de',label:'German edition'},{id:'en',label:'English edition'}],openPane:vi.fn()};
  const contents=mountCorpusContents({reader,provider,locale:()=> 'en'});
  root.querySelector('.cr-contents-button').click();
  const select=root.querySelector('.cr-contents-edition select');
  expect(select.value).toBe('de');select.value='en';select.dispatch('change');
  await Promise.resolve();await Promise.resolve();
  expect(root.querySelector('.cr-contents-list').textContent).toContain('English section');
  finishGerman();await Promise.resolve();await Promise.resolve();
  expect(root.querySelector('.cr-contents-list').textContent).not.toContain('German section');
  root.querySelector('.cr-contents-list button').click();
  expect(reader.openPane).toHaveBeenCalledWith({documentId:'work',versionId:'en',unitId:'en-unit',revision:'en-rev'});
  expect(reader.state().activeVersionId).toBe('de');
  contents.destroy();
});
