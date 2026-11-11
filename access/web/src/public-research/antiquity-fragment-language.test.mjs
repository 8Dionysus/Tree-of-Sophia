import {describe,expect,it} from 'vitest';
import {createFragments,readingLanguages} from './fragments.mjs';

const languages=['ru','en','es','de','zh-Hans'];
describe('Antiquity fragment language',()=>{
  it('chooses the selected language even when another translation is available',()=>{
    for(const language of languages){
      const result=readingLanguages(language,'grc',['grc','ru','en','es','de','zh-Hans'],{exact:true});
      expect(result).toEqual({primary:[language],alternate:'grc'});
    }
    expect(readingLanguages('de','de',['de'],{exact:true})).toEqual({primary:['de'],alternate:null});
    expect(readingLanguages('zh-Hans','grc',['grc','en'],{exact:true})).toEqual({primary:[],alternate:'grc'});
  });
  it('marks a missing translation and keeps the original separately available',async()=>{
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
      click(){this.events.click?.();}
    }
    globalThis.document={createElement:tag=>new Element(tag),body:new Element('body')};
    try{
      const fragments=createFragments({profile:'antiquity',locale:()=> 'ru',t:key=>key});
      const figure=fragments.fragment({w:'de-anima',loc:'I.1 402a6-7'});
      for(let i=0;i<100&&figure.attributes['aria-busy'];i++)await new Promise(resolve=>setTimeout(resolve,5));
      const all=node=>[node,...node.children.flatMap(all)];
      expect(all(figure).map(node=>node.textContent)).toContain('fragmentTranslationUnavailable');
      expect(all(figure).filter(node=>node.tagName==='blockquote')).toHaveLength(0);
      const swap=all(figure).find(node=>node.textContent==='fragmentOriginal');
      expect(swap).toBeDefined();swap.click();
      expect(all(figure).filter(node=>node.tagName==='blockquote').map(node=>node.lang)).toEqual(['grc']);
      all(figure).find(node=>node.textContent==='fragmentTranslation').click();
      expect(all(figure).filter(node=>node.tagName==='blockquote')).toHaveLength(0);
    }finally{globalThis.document=prior;}
  });
});
