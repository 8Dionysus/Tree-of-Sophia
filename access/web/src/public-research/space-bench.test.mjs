import {expect,it} from 'vitest';
import {benchZone,layBench} from './space-bench.mjs';

// A small stand-in for the browser's elements: enough to move parts between trays and to match simple selectors.
class Node{
  constructor(tag,cls=''){this.tagName=tag.toUpperCase();this.className=cls;this.children=[];this.dataset={};this.attributes={};this.textContent='';this.parentNode=null;this.ownerDocument=doc;}
  append(...items){for(const item of items){if(item.parentNode)item.parentNode.children.splice(item.parentNode.children.indexOf(item),1);item.parentNode=this;this.children.push(item);}}
  replaceChildren(...items){for(const child of this.children)child.parentNode=null;this.children=[];this.append(...items);}
  get childElementCount(){return this.children.length;}
  setAttribute(name,value){this.attributes[name]=String(value);}
  getAttribute(name){return this.attributes[name]??null;}
  cloneNode(){const copy=new Node(this.tagName,this.className);Object.assign(copy.dataset,this.dataset);Object.assign(copy.attributes,this.attributes);return copy;}
  matches(list){return list.split(',').some(selector=>{const [tag,...classes]=selector.trim().split('.');const own=this.className.split(/\s+/);return (!tag||tag.toUpperCase()===this.tagName)&&classes.every(name=>own.includes(name));});}
}
const doc={createElement:tag=>new Node(tag)};
const make=(tag,cls='',text='',data={})=>{const node=new Node(tag,cls);node.textContent=text;Object.assign(node.dataset,data);return node;};
const all=node=>[node,...node.children.flatMap(all)];
const texts=node=>all(node).map(item=>item.textContent).filter(Boolean);
const step=(board,name)=>board.children.find(item=>item.dataset.step===name);
const words={source:'Источник',reading:'Прочтение',comparison:'Сопоставление',sourceCaption:'слова текста',readingCaption:'что видит Древо',comparisonCaption:'встреча с другими'};

it('lays a star’s card out by the steps of interpretation, each part at its own step',()=>{
  const content=make('div','ps-panel-content');
  const ladder=make('p','ps-ladder');for(const rung of ['source','reading','comparison'])ladder.append(make('span','ps-rung','',{rung}));ladder.children[1].setAttribute('aria-current','true');
  const authored=make('div','ps-authored');authored.append(make('p','body','Прочтение мысли'),make('figure','ps-fragment','Слова книги'));
  const relations=make('section','ps-card-relations','',{zone:'comparison'});relations.append(make('button','connection-target','Ворота'));
  content.append(make('p','eyebrow','Тип · Ход'),make('h1','','Признать затруднение'),make('details','ps-fact','◷ 387'),ladder,authored,make('section','ps-question-card','Что превратит незнание в ответ?'),relations,make('div','reading-bottom','Сравнить'),make('section','ps-kernel-personal ps-card-work','Моя работа'));
  const board=layBench(content,{words});
  expect(content.children).toEqual([board]);
  expect(board.children.map(item=>item.dataset.step??item.tagName)).toEqual(['HEADER','source','reading','comparison']);
  expect(texts(board.children[0])).toEqual(['Тип · Ход','Признать затруднение']);
  expect(texts(step(board,'source'))).toEqual(['Источник','слова текста','◷ 387','Слова книги']);
  expect(texts(step(board,'reading'))).toEqual(['Прочтение','что видит Древо','Прочтение мысли','Что превратит незнание в ответ?']);
  expect(texts(step(board,'comparison'))).toEqual(['Сопоставление','встреча с другими','Ворота','Сравнить','Моя работа']);
  // The step the star stands on is marked; the ladder itself unfolds into the columns, and each half of the authored
  // text keeps its wrapper.
  expect(step(board,'reading').children[0].getAttribute('aria-current')).toBe('true');
  expect(step(board,'source').children[0].getAttribute('aria-current')).toBeNull();
  expect(all(board).some(item=>item.className==='ps-ladder')).toBe(false);
  for(const name of ['source','reading'])expect(all(step(board,name)).some(item=>item.className==='ps-authored')).toBe(true);
});

it('opens a core card so that its tools stand in the head and its parts at their steps',()=>{
  const content=make('div'),card=make('article','ps-core-card','',{coreId:'recurrence'});
  card.append(make('header','ps-kernel-header','Понятие'),make('nav','ps-kernel-actions','Читать'),make('section','ps-core-formula','Всё идёт, всё возвращается'),make('section','ps-kernel-reference-section','Свидетельства',{zone:'source'}),make('section','ps-kernel-reference-section','Связи',{zone:'comparison'}),make('section','ps-kernel-personal','Моя работа'));
  content.append(make('h1','','Вечное возвращение'),card);
  const board=layBench(content,{words}),[head]=board.children;
  expect(texts(head)).toEqual(['Вечное возвращение','Читать']);expect(head.children[1].className).toBe('ps-bench-tools');
  expect(texts(step(board,'source')).slice(2)).toEqual(['Всё идёт, всё возвращается','Свидетельства']);
  expect(texts(step(board,'reading')).slice(2)).toEqual(['Понятие']);
  expect(texts(step(board,'comparison')).slice(2)).toEqual(['Связи','Моя работа']);
  // Each step keeps a copy of the card around its parts, with the card's own marks.
  for(const name of ['source','reading','comparison'])expect(all(step(board,name)).find(item=>item.className==='ps-core-card').dataset.coreId).toBe('recurrence');
});

it('lays a link between its two stars, with what joins them in the middle at the step of comparison',()=>{
  const content=make('div'),authored=make('div','ps-authored');authored.append(make('p','','Пример меняет положение'),make('figure','ps-fragment','Слова Менона'));
  content.append(make('p','eyebrow','Переход · развивает'),make('h1','','Проверить затруднение на примере'),make('p','ps-ladder'),authored,make('section','ps-endpoint-source','Источник: Менон',{end:'to'}),make('div','reading-actions','Сопоставить'),make('article','ps-bench-end','Найти пример',{end:'from'}),make('article','ps-bench-end','Признать затруднение',{end:'to'}),make('section','ps-kernel-personal','Моя работа'));
  const board=layBench(content,{mode:'edge',words});
  expect(board.dataset.card).toBe('edge');
  expect(board.children.map(item=>item.dataset.step??item.tagName)).toEqual(['HEADER','from','relation','to']);
  expect(texts(board.children[0])).toEqual(['Переход · развивает','Проверить затруднение на примере','Сопоставить']);
  expect(texts(step(board,'from'))).toEqual(['Найти пример']);
  expect(texts(step(board,'relation'))).toEqual(['Сопоставление','встреча с другими','Пример меняет положение','Слова Менона','Моя работа']);
  expect(step(board,'relation').children[0].getAttribute('aria-current')).toBe('true');
  // The star opens its end; what the link draws from that star's source follows it.
  expect(texts(step(board,'to'))).toEqual(['Признать затруднение','Источник: Менон']);
  expect(benchZone(make('figure','ps-fragment'),'edge')).toBe('relation');expect(benchZone(make('figure','ps-fragment'))).toBe('source');
});
