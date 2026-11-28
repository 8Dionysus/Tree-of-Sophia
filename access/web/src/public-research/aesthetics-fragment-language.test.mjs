import {describe,expect,it} from 'vitest';
import {createFragments} from './fragments.mjs';

describe('Aesthetics fragment language',()=>{
  it('shows the original and a clear gap instead of an English fallback',async()=>{
    const prior=globalThis.document;
    class Element {
      constructor(tag){this.tagName=tag;this.children=[];this.dataset={};this.events={};this.attributes={};this.textContent='';}
      append(...items){this.children.push(...items);}
      replaceChildren(...items){this.children=items;this.textContent='';}
      setAttribute(key,value){this.attributes[key]=value;}
      removeAttribute(key){delete this.attributes[key];}
      addEventListener(key,fn){this.events[key]=fn;}
      dispatchEvent(){}
      querySelector(){return null;}
    }
    globalThis.document={createElement:tag=>new Element(tag),body:new Element('body')};
    try{
      const fragments=createFragments({profile:'aesthetics',locale:()=> 'ru',t:key=>key});
      const figure=fragments.fragment({w:'immanuel-kant.kritik-der-urteilskraft',loc:'§1 203.09-15'});
      for(let i=0;i<100&&figure.attributes['aria-busy'];i++)await new Promise(resolve=>setTimeout(resolve,5));
      const all=node=>[node,...node.children.flatMap(all)];
      expect(all(figure).filter(node=>node.tagName==='blockquote').map(node=>node.lang)).toEqual(['de']);
      expect(all(figure).map(node=>node.textContent)).toContain('fragmentTranslationUnavailable');
    }finally{globalThis.document=prior;}
  });
  it('shows the attribution and source for each actual translated witness',async()=>{
    const prior=globalThis.document;
    class Element {
      constructor(tag){this.tagName=tag;this.children=[];this.dataset={};this.events={};this.attributes={};this.textContent='';}
      append(...items){this.children.push(...items);}
      replaceChildren(...items){this.children=items;this.textContent='';}
      setAttribute(key,value){this.attributes[key]=value;}
      removeAttribute(key){delete this.attributes[key];}
      addEventListener(key,fn){this.events[key]=fn;}
      dispatchEvent(){}
      querySelector(){return null;}
    }
    globalThis.document={createElement:tag=>new Element(tag),body:new Element('body')};
    try{
      const fragments=createFragments({profile:'aesthetics',locale:()=> 'en',t:key=>key});
      const figure=fragments.fragment({w:'friedrich-schiller.ueber-das-erhabene',loc:'Essay p286a'});
      for(let i=0;i<100&&figure.attributes['aria-busy'];i++)await new Promise(resolve=>setTimeout(resolve,5));
      const all=node=>[node,...node.children.flatMap(all)];
      const captions=all(figure).filter(node=>node.tagName==='figcaption');
      expect(captions).toHaveLength(2);
      expect(all(captions[1]).map(node=>node.textContent).join(' ')).toContain('translated by John Weiss');
      expect(all(captions[1]).find(node=>node.tagName==='a').href).toBe('https://archive.org/download/stheticletterses01schi/page/n287.jpg');
      expect(all(captions[0]).find(node=>node.tagName==='a').href).not.toBe(all(captions[1]).find(node=>node.tagName==='a').href);
    }finally{globalThis.document=prior;}
  });

});
