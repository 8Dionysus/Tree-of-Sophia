import {describe,expect,it,vi} from 'vitest';
import {readFileSync} from 'node:fs';
import {createTerms,termForms,termMatcher,treeRendering} from './term-view.mjs';

const module={terms:[
  {id:'ubermensch',original:{lemma:'Übermensch',forms:['Übermensch','Übermenschen']},renderings:{ru:{tos:{term:'сверхчеловек',forms:['сверхчеловек','сверхчеловека']},witness:{term:'сверхчеловек',forms:['сверхчеловеке']}},en:{tos:{term:'overman',forms:['overman']},witness:{term:'Superman',forms:['Superman']}}}},
  {id:'kobolde',original:{lemma:'Kobold',forms:['Kobolde']},renderings:{ru:{tos:{term:'горные духи',forms:['горные духи','горных духов']},witness:{term:'горные духи',forms:['горных духов']}},en:{tos:{term:'goblins',forms:['goblins']}}}},
]};

describe('active terms',()=>{
  it('shows the distinct Tree rendering even when its Russian word matches Antonovsky',()=>{
    expect(treeRendering(module.terms[0],'ru')).toEqual({lang:'en',term:'overman'});
    expect(treeRendering(module.terms[0],'en')).toEqual({lang:'en',term:'overman'});
    expect(treeRendering(module.terms[1],'ru')).toBeNull();
    const registry=JSON.parse(readFileSync(new URL('../../../../bonsai/zarathustra/terms.json',import.meta.url),'utf8'));
    const overman=registry.terms.find(term=>term.id==='overman');
    expect(overman.original.lemma).toBe('Übermensch');
    expect(overman.renderings.ru.witness.term).toBe('сверхчеловек');
    expect(overman.renderings.en.witness.term).toBe('Superman');
    expect(treeRendering(overman,'ru')).toEqual({lang:'en',term:'overman'});
    expect(overman.gloss.ru).toBeTruthy();
  });
  it('read the Tree’s words and the shown translation’s words in a language, and the original’s in German',()=>{
    expect([...termForms(module,'ru').keys()]).toEqual(['сверхчеловек','сверхчеловека','сверхчеловеке','горные духи','горных духов']);
    expect([...termForms(module,'en').keys()]).toEqual(['overman','Superman','goblins']);
    expect([...termForms(module,'de').keys()]).toEqual(['Übermensch','Übermenschen','Kobolde']);
  });
  it('find a form as a whole word, the longest first; a lower-case form in any case',()=>{
    const found=termMatcher(termForms(module,'ru')).find('Сверхчеловека ждут горных духов и сверхчеловечество.').map(item=>item.term.id);
    expect(found).toEqual(['ubermensch','kobolde']);
    expect(termMatcher(new Map())).toBeNull();
  });
  it('read a capitalised form with its capital',()=>{
    const terms={terms:[{id:'becoming',original:{lemma:'Werden',forms:['Werden']},renderings:{ru:{tos:{term:'Само',forms:['Само','Самого']},witness:{term:'Само',forms:['Само']}}}}]};
    expect(termMatcher(termForms(terms,'de')).find('Wir werden sehen; das Werden bleibt.').map(item=>item.text)).toEqual(['Werden']);
    expect(termMatcher(termForms(terms,'ru')).find('превзойти его самого; служит своему Самому, Самого.').map(item=>item.text)).toEqual(['Самого']);
    expect(termMatcher(new Map())).toBeNull();
  });
});

it('marks quoted it-was forms without matching ordinary past-tense words',()=>{
  const registry=JSON.parse(readFileSync(new URL('../../../../bonsai/zarathustra/terms.json',import.meta.url),'utf8'));
  const only={terms:registry.terms.filter(term=>term.id==='it-was')};
  expect(termMatcher(termForms(only,'ru')).find('Это было давно. «Было» и «было».').map(x=>x.text)).toEqual(['«Было»','«было»']);
  expect(termMatcher(termForms(only,'en')).find('It was late. “It was” and ‘it was’.').map(x=>x.text)).toEqual(['“It was”','‘it was’']);
});

it('first touch remains a preview when focus arrives before click',()=>{
  const handlers={};
  class Element{
    constructor(tag){this.tagName=tag.toUpperCase();this.dataset={};this.style={};this.attrs={};this.children=[];this.hidden=false;}
    setAttribute(k,v){this.attrs[k]=v;} removeAttribute(k){delete this.attrs[k];}
    append(...nodes){for(const node of nodes){node.parentElement=this;this.children.push(node);}}
    replaceChildren(...nodes){this.children=[];this.append(...nodes);}
    addEventListener(){} remove(){} contains(node){return this===node||this.children.includes(node);}
    closest(selector){return selector==='.ps-term'&&this.dataset.term?this:null;}
    getBoundingClientRect(){return {left:0,width:20,top:20,bottom:40};}
  }
  const body=new Element('body');
  const doc={body,documentElement:{lang:'ru'},createElement:tag=>new Element(tag),addEventListener:(type,fn)=>{handlers[type]=fn;},removeEventListener:()=>{}};
  vi.stubGlobal('document',doc);vi.stubGlobal('innerWidth',800);vi.stubGlobal('innerHeight',600);vi.stubGlobal('addEventListener',()=>{});vi.stubGlobal('removeEventListener',()=>{});
  const open=vi.fn(),terms=createTerms({module:{terms:[{id:'x',original:{lemma:'X'},first:{c:'II.20'}}]},open});
  const span=new Element('span');span.dataset.term='x';
  const event={target:span,pointerType:'touch',preventDefault(){},stopPropagation(){}};
  handlers.pointerdown(event);handlers.focusin(event);handlers.click(event);
  expect(open).not.toHaveBeenCalled();expect(body.children[0].hidden).toBe(false);
  handlers.pointerdown(event);handlers.click(event);expect(open).toHaveBeenCalledOnce();
  terms.dispose();vi.unstubAllGlobals();
});


it('releases observers for removed roots while preserving connected or reattached roots',()=>{
  const observers=[];
  class Observer{
    constructor(callback){this.callback=callback;this.disconnected=false;observers.push(this);}
    observe(target){this.target=target;} disconnect(){this.disconnected=true;}
  }
  const body={append(){}};
  const element=()=>({dataset:{},style:{},setAttribute(){},removeAttribute(){},addEventListener(){},remove(){},querySelectorAll(){return [];},isConnected:true});
  const doc={body,documentElement:{lang:'ru'},createElement:element,createTreeWalker:()=>({nextNode:()=>false}),addEventListener(){},removeEventListener(){}};
  vi.stubGlobal('document',doc);vi.stubGlobal('MutationObserver',Observer);vi.stubGlobal('NodeFilter',{SHOW_TEXT:4});vi.stubGlobal('addEventListener',()=>{});vi.stubGlobal('removeEventListener',()=>{});
  const terms=createTerms({module:{terms:[{id:'x'}]}}),old=element(),live=element();
  terms.mark(old);terms.mark(live);
  const [removals,oldObserver,liveObserver]=observers;
  old.isConnected=false;
  removals.callback([{removedNodes:[{contains:root=>root===old}]}]);
  expect(oldObserver.disconnected).toBe(true);expect(liveObserver.disconnected).toBe(false);
  // Moving a root within the document must not stop future updates.
  removals.callback([{removedNodes:[live]}]);expect(liveObserver.disconnected).toBe(false);
  terms.dispose();expect(removals.disconnected).toBe(true);expect(liveObserver.disconnected).toBe(true);
  vi.unstubAllGlobals();
});
