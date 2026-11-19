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
});
