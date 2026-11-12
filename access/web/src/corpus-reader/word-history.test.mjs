import {readFileSync} from 'node:fs';
import {beforeAll,describe,expect,it} from 'vitest';
import {layerStatus} from '../../../../bonsai/lib/layers.mjs';
import {existsSync} from 'node:fs';
import {frequencyBins,historyRecord,paired,renderAttested,renderInBook,renderStoryText,renderTold,shardOf,storyOrder,storyPiece,storyRecord,toldOrder} from './word-origin.mjs';

// The history of a word: what Wiktionary's editions tell of it, where the German texts of the DTA use it, how often the book says it.
const home=new URL('../../../../bonsai/zarathustra/',import.meta.url);
const read=path=>JSON.parse(readFileSync(new URL(path,home),'utf8'));
// The histories live in R2 with the word layer (bonsai/zarathustra/layers.json); the cases on them run where a checkout fetched them.
const fetched=layerStatus(home.pathname).every(entry=>entry.state==='present');

class Element {
  constructor(tag){this.tagName=tag;this.children=[];this.attributes={};this.dataset={};this.listeners={};this.className='';this.textContent='';this.hidden=false;}
  addEventListener(name,fn){this.listeners[name]=fn;}
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
  it('tells the etymology in the reader\'s language and the original\'s, never in a third',()=>{
    expect(toldOrder({en:told.en,de:told.de,ru:{text:'…'}},'ru','de')).toEqual(['ru','de']);
    expect(toldOrder({en:told.en,de:told.de,ru:{text:'…'}},'en','de')).toEqual(['en','de']);
    // A Russian reader is never told the word in English, nor an English one in Russian.
    expect(toldOrder(told,'ru','de')).toEqual(['de']);
    expect(toldOrder({de:told.de,ru:{text:'…'}},'en','de')).toEqual(['de']);
    expect(toldOrder(told,'de','de')).toEqual(['de']);
    // A reader whose interface has no own copy reads in its copy language (shownLanguages).
    expect(toldOrder(told,'es','de')).toEqual(['en','de']);
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

  const story={ru:[{text:'Übermensch сложено из über «над, сверх» и Mensch «человек».',sources:['W.ru']},
      {text:'Заратустра говорит, что поднял слово на дороге.',sources:['B3','W.ru']},{text:'Сверхчеловек — смысл земли.',sources:['K1']}],
    en:[{text:'Übermensch is made of über- and Mensch.',sources:['W.en']}],
    pieces:{'W.ru':{kind:'wiktionary',edition:'ru',entry:'Übermensch',text:'Образовано из über.'},
      B3:{kind:'book-place',chapter:'III.12',unit:'u-3',start:20,end:31,title:{de:'Von alten und neuen Tafeln.',ru:'О старых и новых скрижалях.',en:'LVI. OLD AND NEW TABLES.'},
        text:{de:'Dort war\'s auch, wo ich das Wort „Übermensch“ vom Wege auflas,',ru:'Там же поднял я на дороге слово «сверхчеловек»,',en:'There was it also where I picked up from the path the word “Superman,”'}},
      K1:{kind:'key-place',chapter:'I.01',unit:'u-1',start:4,end:14,title:{ru:'Предисловие Заратустры.'},text:{de:'Der Übermensch ist der Sinn der Erde.',ru:'Сверхчеловек — смысл земли.'},why:{ru:'Заратустра называет сверхчеловека смыслом земли.'}},
      T:{kind:'translations',words:{ru:[['сострадание',36],['жалость',9]],en:[['pity',41]]},first:{ru:'жалость',en:'pity'},tree:{ru:'сострадание',en:'pity'}}}};
  it('numbers a story\'s pieces in the order it first cites them',()=>{
    expect(storyOrder(story.ru)).toEqual(['W.ru','B3','K1']);
    expect(storyOrder([])).toEqual([]);
  });
  it('tells the story in the reader\'s language with each sentence\'s numbers after it',()=>{
    const parent=new Element('section');
    const shown=renderStoryText(parent,story,{locale:'ru'});
    expect(shown.order).toEqual(['W.ru','B3','K1']);
    const marks=find(parent,'cr-story-mark');
    expect(marks.map(mark=>[mark.textContent,mark.dataset.target])).toEqual([['1','W.ru'],['2','B3'],['1','W.ru'],['3','K1']]);
    expect(find(parent,'cr-story-text')[0].lang).toBe('ru');
    const english=new Element('section');
    renderStoryText(english,story,{locale:'es'});
    expect(find(english,'cr-story-text')[0].lang).toBe('en');
    expect(renderStoryText(new Element('section'),{ru:[],en:[]},{locale:'ru'})).toBe(null);
  });
  it('draws a cited place of the book with the reader\'s translation, the original and its opening',()=>{
    const item=new Element('li'),opened=[];
    expect(storyPiece(item,'B3',story.pieces.B3,{locale:'ru',onOpenPlace:place=>opened.push(place)})).toBe(true);
    expect(find(item,'cr-story-piece-title')[0].text).toBe('О старых и новых скрижалях.');
    expect(find(item,'cr-origin-said')[0].text).toBe('Там же поднял я на дороге слово «сверхчеловек»,');
    expect(find(item,'cr-story-original')[0].lang).toBe('de');
    find(item,'cr-origin-open')[0].listeners.click();
    expect(opened).toEqual([{chapter:'III.12',unitId:'u-3',start:20,end:31,language:'de'}]);
    const key=new Element('li');
    storyPiece(key,'K1',story.pieces.K1,{locale:'ru'});
    expect(find(key,'cr-origin-source')[0].text).toBe('Древо: Заратустра называет сверхчеловека смыслом земли.');
  });
  it('quotes a sentence cut from a speech without the speech\'s lone opening or closing mark',()=>{
    expect(paired('«Я учу вас о сверхчеловеке.','ru')).toBe('Я учу вас о сверхчеловеке.');
    expect(paired('Смех признал я священным; о. высшие люди, научитесь же у меня — смеяться!»','ru')).toBe('Смех признал я священным; о. высшие люди, научитесь же у меня — смеяться!');
    expect(paired('Там же поднял я на дороге слово «сверхчеловек»,','ru')).toBe('Там же поднял я на дороге слово «сверхчеловек»,');
    expect(paired('Dort war\'s, wo ich das Wort „Übermensch“ vom Wege auflas','de')).toBe('Dort war\'s, wo ich das Wort „Übermensch“ vom Wege auflas');
    expect(paired('“I make hymns and sing them;','en')).toBe('I make hymns and sing them;');
    const item=new Element('li');
    storyPiece(item,'K9',{kind:'key-place',chapter:'I.01',title:{ru:'Предисловие Заратустры.'},text:{de:'„Ich lehre euch den Übermenschen.',ru:'«Я учу вас о сверхчеловеке.'}},{locale:'ru'});
    expect(find(item,'cr-origin-said')[0].text).toBe('Я учу вас о сверхчеловеке.');
    expect(find(item,'cr-story-original')[0].text).toBe('Ich lehre euch den Übermenschen.');
  });
  it('names the witness and licence of every sentence a piece quotes',()=>{
    const item=new Element('li');
    storyPiece(item,'B3',story.pieces.B3,{locale:'ru'});
    expect(find(item,'cr-story-witness').map(line=>line.text)).toEqual(['пер. Ю. М. Антоновского, 1913 · CC BY-SA 4.0','первое издание · CC BY-SA 4.0']);
    expect(item.dataset.witnesses).toBe('ru de');
    const english=new Element('li');
    storyPiece(english,'B3',{...story.pieces.B3,text:{...story.pieces.B3.text,en:'There was it also where I picked up from the path the word “Superman,”'}},{locale:'es'});
    expect(find(english,'cr-story-witness')[0].text).toBe('trad. de Thomas Common, 1909 · Public domain');
    expect(english.dataset.witnesses).toBe('en de');
  });
  it('names the words of the reader\'s own translator by their places, the word at the first place apart',()=>{
    const item=new Element('li');
    storyPiece(item,'T',story.pieces.T,{locale:'ru'});
    expect(find(item,'cr-origin-line').map(line=>line.text)).toEqual(['Антоновский: сострадание (36), жалость (9) · на первом месте: жалость','Древо: сострадание']);
    const english=new Element('li');
    storyPiece(english,'T',story.pieces.T,{locale:'en'});
    expect(find(english,'cr-origin-line').map(line=>line.text)).toEqual(['Common: pity (41)','The Tree: pity']);
  });
  it('draws no piece in a language that is neither the original nor the reader\'s',()=>{
    const russian={kind:'wiktionary',edition:'ru',entry:'сверхчеловек',text:'Калька с нем. Übermensch.'};
    const english={kind:'wiktionary',edition:'en',entry:'сверхчеловек',text:'Calque of German Übermensch; from сверх- + человек.'};
    expect(storyPiece(new Element('li'),'V.ru',russian,{locale:'en'})).toBe(false);
    expect(storyPiece(new Element('li'),'V.en',english,{locale:'en'})).toBe(false);
    expect(storyPiece(new Element('li'),'V.ru',russian,{locale:'ru'})).toBe(true);
    expect(storyPiece(new Element('li'),'W.en',{...english,entry:'Übermensch',text:'From über- + Mensch.'},{locale:'ru'})).toBe(false);
    expect(storyPiece(new Element('li'),'W.de',{kind:'wiktionary',edition:'de',entry:'Übermensch',text:'Determinativkompositum.'},{locale:'en'})).toBe(true);
    // A German reader reads the original alone: no translation, no story.
    expect(storyPiece(new Element('li'),'T',story.pieces.T,{locale:'de'})).toBe(false);
    expect(renderStoryText(new Element('div'),{ru:[{text:'Сверхчеловек.',sources:['T']}],en:[{text:'Overman.',sources:['T']}]},{locale:'de'})).toBe(null);
  });
  it.skipIf(!fetched||!existsSync(new URL('stories.json',home)))('holds each story with every piece it cites',async()=>{
    const book=read('stories.json');
    expect(book.schema).toBe('tos.bonsai.stories.v1');
    for(const [lemma,item] of Object.entries(book.stories)){
      for(const language of ['ru','en']){
        expect(item[language].length,`${lemma} ${language}`).toBeGreaterThan(0);
        for(const sentence of item[language])for(const id of sentence.sources)expect(item.pieces[id],`${lemma}: ${id}`).toBeTruthy();
      }
      // A place the reader opens covers its words: an empty range opens nothing.
      for(const [id,piece] of Object.entries(item.pieces))if(piece.unit)expect(piece.end,`${lemma}: ${id}`).toBeGreaterThan(piece.start);
    }
    expect(await storyRecord('Übermensch')).toEqual(book.stories['Übermensch']);
  });
});
