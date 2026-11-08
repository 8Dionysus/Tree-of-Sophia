import {readFileSync} from 'node:fs';
import {beforeAll,describe,expect,it} from 'vitest';
import {layerStatus} from '../../../../bonsai/lib/layers.mjs';
import {frequencyBins,historyRecord,renderAttested,renderInBook,renderTold,shardOf,toldOrder} from './word-origin.mjs';

// The history of a word: what Wiktionary's editions tell of it, where the German texts of the DTA use it, how often the book says it.
const home=new URL('../../../../bonsai/zarathustra/',import.meta.url);
const read=path=>JSON.parse(readFileSync(new URL(path,home),'utf8'));
// The histories live in R2 with the word layer (bonsai/zarathustra/layers.json); the cases on them run where a checkout fetched them.
const fetched=layerStatus(home.pathname).every(entry=>entry.state==='present');

class Element {
  constructor(tag){this.tagName=tag;this.children=[];this.attributes={};this.className='';this.textContent='';}
  append(...items){for(const item of items)this.children.push(typeof item==='string'?new Text(item):item);}
  setAttribute(name,value){this.attributes[name]=String(value);}
  get text(){return this.textContent+this.children.map(child=>child.text).join('');}
}
class Text {constructor(text){this.textContent=text;this.children=[];}get text(){return this.textContent;}}
const all=node=>[node,...node.children.flatMap(all)];
const find=(node,className)=>all(node).filter(item=>item.className===className||item.attributes?.class?.split(' ').includes(className));
beforeAll(()=>{globalThis.document={createElement:tag=>new Element(tag),createElementNS:(ns,tag)=>new Element(tag),createTextNode:text=>new Text(text)};});

const told={en:{text:'From Middle High German mitelīden, first used in mystic literature as a calque of Latin compassiō.',entry:'Mitleid'},
  de:{text:'Mittelhochdeutsch „mitelīden“. Das Wort ist seit dem 14. Jahrhundert belegt.',entry:'Mitleid'}};
const attested={from:1450,n:[0,0,0,0,0,0,0,0,0,0,0,0,1,3,3,1,0,19,29,2,1,0],first:1769,
  early:[[1769,'Johann Gottfried von Herder','Kritische Wälder','… so wird der, so ihn noch beurtheilen und tadeln kann, ein völliger Uebermensch!',[69,80],'herder_kritische02_1769',24,'Uebermenſch']],
  nietzsche:[[1883,'Also sprach Zarathustra']]};
const periods={from:1450,step:25,count:22},nouns=[1699,2063,5684,2711,34848,81029,152924,176539,362972,476085,474974,433600,470462,845262,540740,1181324,1422016,1311962,687658,54470,15137,12218];

describe('the history of a word',()=>{
  it('tells the etymology in the reader\'s language first, then the word\'s own, then English',()=>{
    expect(toldOrder({en:told.en,de:told.de,ru:{text:'…'}},'ru','de')).toEqual(['ru','de','en']);
    expect(toldOrder(told,'ru','de')).toEqual(['de','en']);
    expect(toldOrder(told,'en','de')).toEqual(['en','de']);
    expect(toldOrder(told,'de','de')).toEqual(['de','en']);
    // A reader whose language has no edition reads the word's own edition first.
    expect(toldOrder(told,'es','de')).toEqual(['de','en']);
    expect(toldOrder(told,'zh-Hans','de')).toEqual(['de','en']);
    expect(toldOrder({},'ru','de')).toEqual([]);
  });
  it('shows one telling with its entry, the other editions folded beneath',()=>{
    const parent=new Element('section');
    expect(renderTold(parent,{told},{locale:'en',language:'de'})).toBe(true);
    expect(find(parent,'cr-origin-told')[0].text).toMatch(/mystic literature/);
    expect(find(parent,'cr-origin-link')[0].href).toBe('https://en.wiktionary.org/wiki/Mitleid#German');
    const more=find(parent,'cr-origin-more')[0];
    expect(find(more,'cr-origin-told')[0].text).toMatch(/14\. Jahrhundert/);
    expect(find(more,'cr-origin-link')[0].href).toBe('https://de.wiktionary.org/wiki/Mitleid');
  });
  it('credits the dictionary the Russian edition names, in the edition\'s own words',()=>{
    const parent=new Element('section');
    renderTold(parent,{told:{ru:{text:'Калька с нем. Übermensch.',entry:'сверхчеловек',basis:['словаря М. Фасмера']}}},{locale:'ru',language:'ru'});
    expect(find(parent,'cr-origin-source')[0].text).toBe('русский Викисловарь · сверхчеловек · по данным словаря М. Фасмера');
  });
  it('reads a word\'s uses against the nouns of each quarter century the archive holds enough of',()=>{
    const bins=frequencyBins(attested,periods,nouns);
    expect(bins[0]).toMatchObject({from:1550,to:1574,n:0});
    expect(bins.at(-1)).toMatchObject({from:1925,to:1949});
    expect(bins.find(bin=>bin.from===1875).rate).toBeCloseTo(19*10000/1311962);
    expect(frequencyBins({from:1450,n:[]},periods,nouns)).toEqual([]);
  });
  it('quotes the earliest works with the word marked and the page of the archive linked',()=>{
    const parent=new Element('section');
    expect(renderAttested(parent,{attested},{locale:'ru'})).toBe(true);
    expect(find(parent,'cr-origin-line')[0].text).toBe('Впервые в Немецком текстовом архиве — 1769');
    const said=find(parent,'cr-origin-said')[0];
    expect(said.children[1].tagName).toBe('mark');
    expect(said.children[1].textContent).toBe('Uebermensch');
    const link=find(parent,'cr-origin-link')[0];
    expect(link.href).toBe(`https://www.deutschestextarchiv.de/book/view/herder_kritische02_1769/?p=24&hl=${encodeURIComponent('Uebermenſch')}`);
    expect(link.text).toBe('Немецкий текстовый архив, с. 24');
    expect(find(parent,'cr-origin-line').at(-1).text).toBe('У Ницше в архиве: Also sprach Zarathustra, 1883');
    expect(renderAttested(new Element('section'),{},{locale:'ru'})).toBe(false);
  });
  it('names the quarter century of the first uses where their works are not at hand',()=>{
    const parent=new Element('section');
    renderAttested(parent,{attested:{from:1450,n:[1],period:[1750,1774]}},{locale:'en'});
    expect(find(parent,'cr-origin-line')[0].text).toBe('First in the German Text Archive: 1750–1774');
  });
  it('counts the book\'s uses part by part',()=>{
    const parent=new Element('section');
    renderInBook(parent,{book:{n:48,parts:{I:27,II:14,III:3,IV:4}}},{locale:'ru'});
    expect(parent.children[0].text).toBe('В книге — 48 раз · часть I: 27 · часть II: 14 · часть III: 3 · часть IV: 4');
    const once=new Element('section');
    renderInBook(once,{book:{n:1,parts:{IV:1}}},{locale:'en'});
    expect(once.children[0].text).toBe('In the book: once');
    const few=new Element('section');
    renderInBook(few,{book:{n:3,parts:{I:3}}},{locale:'ru'});
    expect(few.children[0].text).toBe('В книге — 3 раза');
  });
  it.skipIf(!fetched)('holds the history of the stand\'s words from the dictionaries and the archive',async()=>{
    const manifest=read('histories.json');
    expect(manifest.schema).toBe('tos.bonsai.histories.v1');
    expect(manifest.attested.nouns).toHaveLength(manifest.attested.periods.count);
    const shard=(language,lemma)=>read(`histories/${language}-${String(shardOf(lemma)).padStart(2,'0')}.json`).lemmas[lemma];
    // Herder calls a man ein völliger Uebermensch in 1769; Goethe's Faust has Übermenschen in 1790; Nietzsche makes it his word in 1883.
    const overman=shard('de','Übermensch');
    expect(overman.attested.first).toBe(1769);
    expect(overman.attested.early.map(use=>use[0]).slice(0,2)).toEqual([1769,1790]);
    for(const [,, ,text,[start,end]] of overman.attested.early)expect(text.slice(start,end)).toMatch(/^(?:Ue|Ü)bermensch/);
    expect(overman.attested.nietzsche).toContainEqual([1883,'Also sprach Zarathustra']);
    expect(overman.book.n).toBeGreaterThan(40);
    expect(shard('de','Mitleid').told.en.text).toMatch(/mystic literature/);
    const calque=shard('ru','сверхчеловек');
    expect(calque.told.ru.text).toMatch(/Übermensch/);
    expect(calque.told.ru.basis).toContain('словаря М. Фасмера');
    expect(await historyRecord('de','Übermensch')).toEqual(overman);
  });
});
