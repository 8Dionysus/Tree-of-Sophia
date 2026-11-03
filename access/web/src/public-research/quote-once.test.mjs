import {expect,it,vi} from 'vitest';
import {keepQuotesOnce} from './quote-once.mjs';

class Node{
  constructor(tag,text=''){this.tagName=tag.toUpperCase();this.children=[];this.parent=null;this.text=text;this.hidden=false;this.attrs={};this.listeners={};const set=new Set();this.classList={add:x=>set.add(x),remove:x=>set.delete(x),contains:x=>set.has(x)};this.className='';}
  append(...items){for(const item of items){item.parent=this;this.children.push(item);}}
  prepend(item){item.parent=this;this.children.unshift(item);}
  remove(){if(this.parent)this.parent.children=this.parent.children.filter(child=>child!==this);}
  get textContent(){return this.text+this.children.map(child=>child.textContent).join('');}
  set textContent(value){this.text=value;}
  getAttribute(name){return this.attrs[name]??null;}
  addEventListener(type,fn){this.listeners[type]=fn;}
  closest(selector){let node=this;while(node){if(selector==='[lang]'&&node.attrs.lang)return node;node=node.parent;}return null;}
  matches(selector){return selector.split(',').some(part=>{part=part.trim().replace(':scope > ','');const [tag,cls]=part.split('.');return (!tag||tag==='*'||this.tagName===tag.toUpperCase())&&(!cls||this.className.split(' ').includes(cls));});}
  all(){return this.children.flatMap(child=>[child,...child.all()]);}
  querySelectorAll(selector){const direct=selector.includes(':scope >');return (direct?this.children:this.all()).filter(node=>node.matches(selector));}
  querySelector(selector){return this.querySelectorAll(selector)[0]??null;}
  get isConnected(){return true;}
}
const figure=(lines,source)=>{const f=new Node('figure');f.className='ps-fragment';const q=new Node('blockquote');q.className='ps-fragment-text';q.attrs.lang='ru';q.append(...lines.map(line=>new Node('p',line)));const c=new Node('figcaption');const s=new Node('span',source);s.className='ps-fragment-source';c.append(s);f.append(q,c);return f;};

it('quotes every line once and leads a whole repeat back to the first quotation',()=>{
  vi.stubGlobal('document',{createElement:tag=>new Node(tag)});
  const root=new Node('article'),a=figure(['«Было» — так называется скрежет зубовный.','Обратно не может воля хотеть.'],'Об избавлении'),b=figure(['Обратно не может воля хотеть.','Ах, безумцем становится каждый пленник!'],'Об избавлении'),c=figure(['«Было» — так называется скрежет зубовный.'],'Об избавлении');
  root.append(a,b,c);
  keepQuotesOnce(root,{label:source=>`↑ ${source} — приведено выше`});
  expect(b.children[0].children.map(line=>line.classList.contains('is-quoted-above'))).toEqual([true,false]);
  expect(c.children[0].textContent).toBe('↑ Об избавлении — приведено выше');
  expect(c.children[1].hidden).toBe(true);
  expect(a.children[0].children.some(line=>line.classList.contains('is-quoted-above'))).toBe(false);
  vi.unstubAllGlobals();
});
