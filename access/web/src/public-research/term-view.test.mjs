import {describe,expect,it,vi} from 'vitest';
import {readFileSync} from 'node:fs';
import {createTerms,linkedTerms,termContextAllows,termForms,termMatcher,termNodes,treeRendering,witnessLines} from './term-view.mjs';
import {PUBLIC_LOCALES} from './locale.mjs';
import {attachCore} from './core-projection.mjs';

const module={terms:[
  {id:'ubermensch',original:{lemma:'Übermensch',forms:['Übermensch','Übermenschen']},renderings:{ru:{tos:{term:'сверхчеловек',forms:['сверхчеловек','сверхчеловека']},witness:{term:'сверхчеловек',forms:['сверхчеловеке']}},en:{tos:{term:'overman',forms:['overman']},witness:{term:'Superman',forms:['Superman']}}}},
  {id:'kobolde',original:{lemma:'Kobold',forms:['Kobolde']},renderings:{ru:{tos:{term:'горные духи',forms:['горные духи','горных духов']},witness:{term:'горные духи',forms:['горных духов']}},en:{tos:{term:'goblins',forms:['goblins']}}}},
]};

describe('active terms',()=>{
  it('shows the Tree’s word only in the reader’s own language, where its translation says another',()=>{
    expect(treeRendering(module.terms[0],'ru')).toBeNull();
    expect(treeRendering(module.terms[0],'en')).toEqual({lang:'en',term:'overman'});
    expect(treeRendering(module.terms[1],'ru')).toBeNull();
    const registry=JSON.parse(readFileSync(new URL('../../../../bonsai/zarathustra/terms.json',import.meta.url),'utf8'));
    const overman=registry.terms.find(term=>term.id==='overman');
    expect(overman.original.lemma).toBe('Übermensch');
    expect(overman.renderings.ru.witness.term).toBe('сверхчеловек');
    expect(overman.renderings.en.witness.term).toBe('Superman');
    expect(treeRendering(overman,'ru')).toBeNull();
    expect(treeRendering(overman,'en')).toEqual({lang:'en',term:'overman'});
    expect(overman.gloss.ru).toBeTruthy();
  });
  it('gives a reader the Tree’s word in their language when no shown translation says it there',()=>{
    const episteme={renderings:{ru:{tos:{term:'знание'}},en:{tos:{term:'knowledge'},witness:[{edition:'lamb',term:'knowledge'},{edition:'fowler',term:'Knowledge'}]}}};
    expect(treeRendering(episteme,'ru')).toEqual({lang:'ru',term:'знание'});
    expect(treeRendering(episteme,'en')).toBeNull();
    const psyche={renderings:{ru:{tos:{term:'душа'},witness:[{edition:'karpov',term:'душа'}]},en:{tos:{term:'soul'},witness:[{edition:'fowler',term:'soul'}]}}};
    expect(treeRendering(psyche,'ru')).toBeNull();
  });
  it('gives one line per translated word with the editions that use it',()=>{
    const editions={lamb:{label:{ru:'Платон · 1924 · У. Р. М. Лэмб',en:'Plato · 1924 · W. R. M. Lamb'},short:{ru:'Лэмб 1924',en:'Lamb 1924'}},perseus:{label:{ru:'Платон · Perseus',en:'Plato · Perseus'},short:{ru:'Лэмб 1924',en:'Lamb 1924'}},fowler:{label:{ru:'Платон · 1921 · Г. Н. Фаулер',en:'Plato · 1921 · H. N. Fowler'},short:{ru:'Фаулер 1921',en:'Fowler 1921'}},bury:{label:{ru:'Платон · 1929 · Р. Г. Бери',en:'Plato · 1929 · R. G. Bury'}}};
    const lines=witnessLines({witness:[{edition:'lamb',term:'knowledge'},{edition:'perseus',term:'knowledge'},{edition:'fowler',term:'Knowledge'},{edition:'bury',term:'science'}]},editions,'ru');
    expect(lines.map(line=>[line.term,line.source])).toEqual([['knowledge','Лэмб 1924, Фаулер 1921'],['science','Платон · 1929 · Р. Г. Бери']]);
    expect(lines[0].title).toBe('Платон · 1924 · У. Р. М. Лэмб\nПлатон · Perseus\nПлатон · 1921 · Г. Н. Фаулер');
    const registry=JSON.parse(readFileSync(new URL('../../../../bonsai/antiquity/terms.json',import.meta.url),'utf8'));
    for(const [id,edition] of Object.entries(registry.editions))expect([id,edition.short?.ru&&edition.short?.en]).toEqual([id,expect.any(String)]);
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
  it('matches Greek accents and breathings while preserving the spelling in the text',()=>{
    const term={id:'logos',original:{lang:'grc',forms:['λόγος']}};
    const matcher=termMatcher(termForms({terms:[term]},'grc'),{greek:true});
    expect(matcher.find('ὁ λόγος, λο\u0301γος, διαλόγος').map(item=>item.text)).toEqual(['λόγος','λο\u0301γος']);
  });
  it('includes the forms of each translation edition',()=>{
    const term={id:'appearance',renderings:{en:{tos:{forms:['impression']},witness:[{forms:['appearance']},{forms:['semblance']}]}}};
    expect([...termForms({terms:[term]},'en').keys()]).toEqual(['impression','appearance','semblance']);
  });
  it('limits Antiquity witness words to their edition and lets that edition resolve collisions',()=>{
    const module={work:'antiquity',terms:[
      {id:'officium',renderings:{en:{tos:{forms:['duty']}}}},
      {id:'kathekon',renderings:{en:{tos:{forms:['appropriate action']},witness:[{edition:'epictetus',forms:['duty']}]}}},
    ]};
    expect(termForms(module,'en').get('duty').id).toBe('officium');
    expect(termForms(module,'en',{edition:'epictetus'}).get('duty').id).toBe('kathekon');
    expect(termForms(module,'en',{edition:'another'}).get('duty').id).toBe('officium');
  });
  it('keeps the English noun will and the mean apart from ordinary verbs',()=>{
    const module={work:'antiquity'},allows=(s,word)=>termContextAllows(module,'en',s,{text:word,index:s.indexOf(word)});
    expect(allows('We will return to the text.','will')).toBe(false);
    expect(allows('Each person’s will gives movement its start.','will')).toBe(true);
    expect(allows('What does this mean?','mean')).toBe(false);
    expect(allows('Virtue observes the mean.','mean')).toBe(true);
  });
  it('uses the cited work to distinguish an ordinary word from an evidenced term',()=>{
    const module={work:'antiquity'},match={text:'роль',index:0,term:{id:'persona',contextWorks:['de-officiis']}};
    expect(termContextAllows(module,'ru','роль гражданина',match,['politics'])).toBe(false);
    expect(termContextAllows(module,'ru','роль человека',match,['de-officiis'])).toBe(true);
    expect(termContextAllows(module,'la','persona',match,['politics'])).toBe(true);
  });
});

it('marks quoted it-was forms without matching ordinary past-tense words',()=>{
  const registry=JSON.parse(readFileSync(new URL('../../../../bonsai/zarathustra/terms.json',import.meta.url),'utf8'));
  const only={terms:registry.terms.filter(term=>term.id==='it-was')};
  expect(termMatcher(termForms(only,'ru')).find('Это было давно. «Было» и «было».').map(x=>x.text)).toEqual(['«Было»','«было»']);
  expect(termMatcher(termForms(only,'en')).find('It was late. “It was” and ‘it was’.').map(x=>x.text)).toEqual(['“It was”','‘it was’']);
});

// A minimal element for the card: enough to read what it says and press its button.
const fakeElement=()=>class Element{
  constructor(tag){this.tagName=tag.toUpperCase();this.dataset={};this.style={};this.attrs={};this.children=[];this.listeners={};this.hidden=false;}
  setAttribute(k,v){this.attrs[k]=v;} removeAttribute(k){delete this.attrs[k];}
  append(...nodes){for(const node of nodes){node.parentElement=this;this.children.push(node);}}
  replaceChildren(...nodes){this.children=[];this.append(...nodes);}
  addEventListener(type,fn){this.listeners[type]=fn;} remove(){} contains(node){return this===node||this.children.includes(node);}
  closest(selector){return selector==='.ps-term'&&this.dataset.term?this:null;}
  getBoundingClientRect(){return {left:0,width:20,top:20,bottom:40};}
};
const stubPage=(lang='ru')=>{
  const handlers={},Element=fakeElement(),body=new Element('body');
  vi.stubGlobal('document',{body,documentElement:{lang},createElement:tag=>new Element(tag),addEventListener:(type,fn)=>{handlers[type]=fn;},removeEventListener(){}});
  vi.stubGlobal('innerWidth',800);vi.stubGlobal('innerHeight',600);vi.stubGlobal('addEventListener',()=>{});vi.stubGlobal('removeEventListener',()=>{});
  const word=(term,node)=>{const span=new Element('span');span.dataset.term=term;span.dataset.node=node;return span;};
  return {handlers,body,word};
};
const all=node=>[node,...node.children.flatMap(all)],text=node=>all(node).map(item=>item.textContent??'').join(' ');

it('leads a term to the node it names and leaves a term without a node as plain text',()=>{
  const nodeOf=term=>term.concept?`node:${term.concept}`:null;
  const {module:linked,nodes}=linkedTerms({work:'zarathustra',terms:[{id:'overman',concept:'overman'},{id:'self'}]},nodeOf);
  expect(linked.terms.map(term=>term.id)).toEqual(['overman']);expect(linked.work).toBe('zarathustra');
  expect([...nodes]).toEqual([['overman','node:overman']]);
  stubPage();
  const none=createTerms({module:{terms:[{id:'self'}]},nodeOf});
  expect(none.size).toBe(0);expect(document.body.children).toEqual([]);
  none.mark({});none.dispose();vi.unstubAllGlobals();
});

it('names the node in Zarathustra’s core and in the Antiquity library',()=>{
  const read=(stand,name)=>JSON.parse(readFileSync(new URL(`../../../../bonsai/${stand}/${name}.json`,import.meta.url),'utf8'));
  const core=Object.fromEntries(['concepts','figures','links','chains','proposed'].map(name=>[name,read('zarathustra',`core/${name}`)]));
  const library={nodes:[{id:'work',kind:'work'}],atlas:{defaultIds:['work'],edges:[]},space:{routes:[]}};
  const {materialIds}=attachCore(library,core).space.core,held=new Set(library.nodes.map(node=>node.id));
  const zarathustra=read('zarathustra','terms'),zarathustraNode=termNodes({stand:'zarathustra',materialIds,has:id=>held.has(id)});
  // Every concept a term names is a node of the core; a term that names none has no link.
  for(const term of zarathustra.terms)expect([term.id,!!zarathustraNode(term)]).toEqual([term.id,!!term.concept]);
  const records=new Set([...core.concepts,...core.figures].map(record=>record.id));
  for(const term of zarathustra.terms.filter(term=>term.concept))expect([term.id,records.has(term.concept)]).toEqual([term.id,true]);
  const collection=read('antiquity','collection'),antiquity=read('antiquity','terms');
  const catalog=new Set([...collection.entities.map(entity=>`antiquity:catalog:${entity.id}`),...collection.terms.map(term=>`antiquity:catalog:lexical:${term.id}`)]);
  const antiquityNode=termNodes({stand:'antiquity',has:id=>catalog.has(id)});
  for(const term of antiquity.terms)expect([term.id,antiquityNode(term)]).toEqual([term.id,term.concept?`antiquity:catalog:${term.concept}`:`antiquity:catalog:lexical:${term.id}`]);
});

it('shows where a word leads: the node’s kind, title and opening line, and opens it',()=>{
  const {handlers,body,word}=stubPage('en');
  const openNode=vi.fn(),describe=vi.fn(id=>id==='antiquity:catalog:lexical:phantasia'?{kind:'Concept',title:'Impression',lead:'An impression meets judgment.'}:null);
  const terms=createTerms({locale:()=> 'en',nodeOf:term=>`antiquity:catalog:lexical:${term.id}`,describe,openNode,
    module:{work:'antiquity',terms:[{id:'phantasia',original:{lang:'grc',lemma:'φαντασία'},renderings:{en:{tos:{term:'impression'}}},gloss:{en:'A gloss of the philological core.'}}]}});
  const span=word('phantasia','antiquity:catalog:lexical:phantasia');handlers.focusin({target:span});
  const card=body.children[0];
  expect(card.dataset.stand).toBe('antiquity');expect(card.hidden).toBe(false);
  expect(all(card).map(node=>node.className).filter(Boolean)).toEqual(['ps-term-card','ps-term-kind','ps-term-star','ps-term-title','ps-term-lead','ps-term-open']);
  expect(text(card)).toContain('Concept');expect(text(card)).toContain('Impression');expect(text(card)).toContain('An impression meets judgment.');
  // The card speaks of the node; the words of the original and the translations belong to the philological core.
  expect(text(card)).not.toContain('φαντασία');expect(text(card)).not.toContain('A gloss of the philological core.');
  expect(span.dataset.shown).toBe('true');expect(span.attrs['aria-describedby']).toBe(card.id);
  card.children.at(-1).listeners.click();
  expect(openNode).toHaveBeenCalledWith('antiquity:catalog:lexical:phantasia');expect(card.hidden).toBe(true);expect(span.dataset.shown).toBeUndefined();
  terms.dispose();vi.unstubAllGlobals();
});

it('a first touch shows the node; the next one follows the link, as does a click',()=>{
  const {handlers,body,word}=stubPage();
  const openNode=vi.fn(),terms=createTerms({module:{terms:[{id:'x'}]},nodeOf:()=> 'node:x',describe:()=>({kind:'Понятие',title:'Икс',lead:''}),openNode});
  const span=word('x','node:x'),touch={target:span,pointerType:'touch',preventDefault(){},stopPropagation(){}};
  handlers.pointerdown(touch);handlers.focusin(touch);handlers.click(touch);
  expect(openNode).not.toHaveBeenCalled();expect(body.children[0].hidden).toBe(false);
  handlers.pointerdown(touch);handlers.click(touch);expect(openNode).toHaveBeenCalledWith('node:x');
  const mouse={target:span,pointerType:'mouse',button:0,preventDefault:vi.fn(),stopPropagation(){}};
  handlers.pointerdown(mouse);handlers.click(mouse);expect(openNode).toHaveBeenCalledTimes(2);expect(mouse.preventDefault).toHaveBeenCalled();
  // A link opened in a new tab keeps the browser's own way.
  span.href='https://example.org/?item=node:x';const tab={...mouse,metaKey:true,preventDefault:vi.fn()};
  handlers.click(tab);expect(openNode).toHaveBeenCalledTimes(2);expect(tab.preventDefault).not.toHaveBeenCalled();
  terms.dispose();vi.unstubAllGlobals();
});

it('speaks the reader’s interface language on the node card, for every term of both stands',()=>{
  for(const stand of ['zarathustra','antiquity']){
    const registry=JSON.parse(readFileSync(new URL(`../../../../bonsai/${stand}/terms.json`,import.meta.url),'utf8'));
    for(const locale of PUBLIC_LOCALES){
      const {handlers,body,word}=stubPage(locale);
      const terms=createTerms({locale:()=>locale,nodeOf:term=>`node:${term.id}`,describe:id=>({kind:'K',title:id,lead:''}),module:{...registry,work:stand==='antiquity'?'antiquity':registry.work}});
      expect(terms.size).toBe(registry.terms.length);
      const card=body.children[0];
      for(const term of registry.terms){
        handlers.focusin({target:word(term.id,`node:${term.id}`)});
        expect(text(card),`${stand} ${locale} ${term.id}`).toContain(`node:${term.id}`);
        if(locale!=='ru')expect(text(card),`${stand} ${locale} ${term.id}`).not.toMatch(/[Ѐ-ӿ]/u);
      }
      terms.dispose();vi.unstubAllGlobals();
    }
  }
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
  const terms=createTerms({module:{terms:[{id:'x'}]},nodeOf:()=> 'node:x'}),old=element(),live=element();
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
