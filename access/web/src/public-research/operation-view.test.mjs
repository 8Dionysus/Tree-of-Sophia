import {beforeAll,expect,it,vi} from 'vitest';
import {readFileSync} from 'node:fs';
import * as rules from '../../../deploy/cloudflare-worker/generated/tos_web_rules.js';
import {configureResearchOperations} from './research-operations.mjs';
import {mountResearchOperations} from './operation-view.mjs';
class Element {
  constructor(tag){this.tagName=tag;this.children=[];this.listeners={};this.classList={add(){},remove(){}};this.value='';this.textContent='';this.attributes={};}
  append(...children){this.children.push(...children);if(this.tagName==='select'&&!this.value&&this.options.length)this.value=this.options[0].value;}
  replaceChildren(...children){this.children=[];this.value='';this.append(...children);}
  get firstChild(){return this.children[0];}
  get options(){return this.children.filter(child=>child.tagName==='option');}
  setAttribute(name,value){this.attributes[name]=value;}
  addEventListener(name,fn){this.listeners[name]=fn;}
  dispatch(name){return this.listeners[name]?.({preventDefault(){}});}
}
const descendants=root=>[root,...root.children.flatMap(child=>child instanceof Element?descendants(child):[])];
const find=(root,tag,predicate=()=>true)=>descendants(root).find(el=>el.tagName===tag&&predicate(el));
beforeAll(()=>{globalThis.document={createElement:tag=>new Element(tag),createTextNode:text=>({textContent:text})};rules.initSync({module:new WebAssembly.Module(readFileSync(new URL('../../../deploy/cloudflare-worker/generated/tos_web_rules_bg.wasm',import.meta.url)))});configureResearchOperations(rules);});
it('calculates from selected records, saves and restores the same reproducible request',async()=>{
  const values=[{id:'a',kind:'concept',title:'A',dependencies:['p']},{id:'b',kind:'concept',title:'B',dependencies:['p']},
    {id:'p',kind:'passage',title:'Passage',record:{c:'I.01'},dependencies:[]},
    {id:'edge',kind:'relation',title:'connects',record:{from:'a',to:'b',relation:'supports',verb:'connects'},dependencies:['a','b','p']},
    {id:'chain',kind:'chain',title:'Chain',record:{steps:[{id:'step',from:'a',to:'b',verb:'connects'}]},dependencies:['a','b']},
    {id:'bestowing-virtue',kind:'concept',title:'Bestowing virtue',record:{},dependencies:[]}].map(item=>({record:{},revision:'v1',...item,dependencies:item.dependencies.map(id=>({id}))}));
  const items=new Map(values.map(x=>[x.id,x])),index={items,revision:'v1',get:id=>items.get(id),resolveId:id=>id,references:id=>values.filter(item=>item.dependencies.some(reference=>reference.id===id))};
  const stored=new Map(),notebook={setPreference:async(k,v)=>stored.set(k,v),exportPacket:async()=>({preferences:[...stored].map(([key,value])=>({key,value}))})};
  const root=new Element('div'),onOpen=vi.fn(),view=mountResearchOperations({root,index,notebook,locale:'en',getSelection:()=> 'a',onOpen,timeOf:()=>null,placeOf:()=>null});
  const operation=find(root,'select');operation.value='common_grounds';operation.dispatch('change');
  const b=find(root,'select',el=>el.attributes['aria-label']==='Second item');b.value='b';b.dispatch('change');
  await find(root,'form').dispatch('submit');expect(view.getLastResult().result.common).toEqual(['p']);
  const open=find(root,'button',el=>el.textContent==='Passage · passage');open.dispatch('click');expect(onOpen).toHaveBeenCalledWith('p');
  await find(root,'button',el=>el.textContent==='Save calculation').dispatch('click');expect(stored.size).toBe(1);
  const original=view.getLastResult().request;
  for(const name of ['directed_path','distribution','interpretation_comparison','reading_distance','commitment_order','argument_structure','signed_support','dependency_impact','time_place']){
    operation.value=name;operation.dispatch('change');
    if(name==='directed_path'||name==='time_place') {const other=find(root,'select',el=>el.attributes['aria-label']==='Second item');other.value='b';other.dispatch('change');}
    await find(root,'form').dispatch('submit');expect(view.getLastResult().request.operation).toBe(name);
  }
  view.destroy();
  const reopened=new Element('div'),second=mountResearchOperations({root:reopened,index,notebook,locale:'en'});await new Promise(resolve=>setTimeout(resolve,0));
  const saved=find(reopened,'button',el=>el.textContent.startsWith('Common grounds ·'));expect(saved).toBeDefined();saved.dispatch('click');expect(second.getLastResult().request).toEqual(original);second.destroy();
});
