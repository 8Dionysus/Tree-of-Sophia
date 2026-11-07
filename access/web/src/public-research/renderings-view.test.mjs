import {readFileSync} from 'node:fs';
import {beforeAll,describe,expect,it} from 'vitest';
import {conceptRenderings,renderRenderings,translationLanguage,wordRenderings} from './renderings-view.mjs';

// How each translation renders each term of the book, place by place.
const read=path=>JSON.parse(readFileSync(new URL(`../../../../bonsai/zarathustra/${path}`,import.meta.url),'utf8'));
const renderings=read('renderings.json'),terms=read('terms.json');

class Element {
  constructor(tag){this.tagName=tag;this.children=[];this.listeners={};this.dataset={};this.style={};this.textContent='';this.open=false;}
  append(...items){this.children.push(...items);}
  setAttribute(){}
  addEventListener(name,fn){this.listeners[name]=fn;}
  get childElementCount(){return this.children.length;}
  toggle(){this.open=true;this.listeners.toggle?.();}
}
const all=node=>[node,...node.children.flatMap(all)];
beforeAll(()=>{globalThis.document={createElement:tag=>new Element(tag)};});

describe('the renderings of a term',()=>{
  it('counts each place of the original once, under the word its aligned sentence uses',()=>{
    const going=renderings.terms['down-going'];
    expect(going.lemma).toBe('Untergang');
    expect(going.ru.words.slice(0,2).map(([word,places])=>[word,places.length])).toEqual([['гибель',10],['закат',4]]);
    const counted=[...going.ru.words.flatMap(([,places])=>places),...going.ru.other];
    // A place whose sentence has no aligned sentence in this translation is not counted for it.
    expect(counted.length).toBeLessThanOrEqual(going.n);
    expect(new Set(counted.map(place=>place.join('|'))).size).toBe(counted.length);
  });
  it('finds the terms of a selected word: by form in the original, by lemma in a translation',()=>{
    const ids=found=>found.map(entry=>entry.lemma);
    expect(ids(wordRenderings(renderings,terms,{language:'de',form:'Untergange'}))).toEqual(['Untergang']);
    expect(ids(wordRenderings(renderings,terms,{language:'ru',form:'гибели',lemma:'гибель'}))).toContain('Untergang');
    expect(ids(wordRenderings(renderings,terms,{language:'en',form:'Superman',lemma:'Superman'}))).toContain('Übermensch');
    expect(wordRenderings(renderings,terms,{language:'ru',form:'стола',lemma:'стол'})).toEqual([]);
    expect(conceptRenderings(renderings,'overman').map(entry=>entry.lemma)).toEqual(['Übermensch']);
  });
  it('shows the reader\'s translation beside the original',()=>{
    expect(translationLanguage('ru')).toBe('ru');
    expect(translationLanguage('en')).toBe('en');
  });
  it('draws one band per word as wide as its places and opens each word\'s places',()=>{
    const going=renderings.terms['down-going'],cites=[];
    const root=new Element('div');
    expect(renderRenderings(root,going,{language:'ru',locale:'ru',fragment:cite=>{cites.push(cite);return new Element('figure');}})).toBe(true);
    const bands=all(root).find(node=>node.className==='ps-renderings-bar').children;
    expect(bands.map(band=>[band.textContent,band.style.flexGrow])).toEqual([...going.ru.words.map(([word,places])=>[word,String(places.length)]),['иначе',String(going.ru.other.length)]]);
    const folds=all(root).filter(node=>node.tagName==='details');
    expect(folds[0].children[0].textContent).toBe('гибель · 10');
    expect(cites).toHaveLength(0);
    folds[0].toggle();
    const [c,unit,s,e]=going.ru.words[0][1][0];
    expect(cites).toHaveLength(10);
    expect(cites[0]).toEqual({w:'also-sprach-zarathustra',c,u:[unit,unit],s,e});
    expect(renderRenderings(new Element('div'),going,{language:'grc',locale:'ru',fragment:()=>null})).toBe(false);
  });
});
