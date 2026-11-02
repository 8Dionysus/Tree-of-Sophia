import {afterEach,expect,test,vi} from 'vitest';
import {mountKnowing} from './knowing-map.mjs';
class Element{
 constructor(tag){this.tagName=tag;this.children=[];this.listeners={};this.attributes={};this.dataset={};this.style={setProperty(){}};this.classes=new Set();this.classList={add:(...xs)=>xs.forEach(x=>this.classes.add(x)),remove:(...xs)=>xs.forEach(x=>this.classes.delete(x)),contains:x=>this.classes.has(x),toggle:(x,v)=>v?this.classes.add(x):this.classes.delete(x)};this.textContent='';}
 set className(value){this._className=value;for(const x of value.split(' '))this.classes.add(x);} get className(){return this._className;}
 append(...items){this.children.push(...items);} replaceChildren(...items){this.children=[...items];} setAttribute(k,v){this.attributes[k]=v;if(k==='class')this.className=v;} addEventListener(k,fn){this.listeners[k]=fn;} querySelector(sel){return all(this).find(x=>x.classes.has(sel.slice(1)))??null;} get childElementCount(){return this.children.length;} get firstChild(){return this.children[0];}
}
const all=node=>[node,...node.children.flatMap(all)];const text=node=>all(node).map(n=>n.textContent).join(' ');
const live=[];afterEach(()=>{live.splice(0).forEach(x=>x.destroy());vi.unstubAllGlobals();});
function fixture(){
 vi.stubGlobal('document',{createElement:t=>new Element(t),createElementNS:(_,t)=>new Element(t)});vi.stubGlobal('ResizeObserver',class{observe(){}});vi.stubGlobal('requestAnimationFrame',()=>0);
 const nodes=['lion','child','other'].map(id=>({id,owner:'bonsai/core',kind:'figure',title:{en:id},record:{title:{en:id}}}));
 const link={id:'child-lion',kind:'relation',record:{admission:'map',from:'child',to:'lion',verb:{en:'becomes'},question:{en:'How?'},grounds:[]}};
 const second={id:'other-lion',kind:'relation',record:{admission:'map',from:'other',to:'lion',verb:{en:'asks'},grounds:[]}};
 let stored={level:'light',start:'lion',known:['lion'],found:[],hypotheses:[]};const copy=x=>structuredClone(x);
 const notebook={getPreference:async()=>copy(stored),setPreference:async(_,value,{expectedValue})=>{if(JSON.stringify(stored)!==JSON.stringify(expectedValue))throw Object.assign(Error('conflict'),{code:'conflict'});stored=copy(value);}};
 const root=new Element('div'),index={items:new Map([...nodes,link,second].map(x=>[x.id,x]))};
 const view=mountKnowing({root,index,locale:()=> 'en',notebook,profile:'test',fragments:{fragment:()=>new Element('p')}});live.push(view);
 return {root,view,stored:()=>copy(stored),remote:value=>stored=copy(value)};
}
test('hovering a threshold keeps its row and expanded journal mounted',async()=>{
 const f=fixture();await f.view.ready;const row=all(f.root).find(x=>x.className==='k-row');const box=all(f.root).find(x=>x.tagName==='details'&&text(x).includes('Questions at the threshold'));box.open=false;
 row.listeners.mouseenter();expect(all(f.root)).toContain(row);expect(all(f.root)).toContain(box);expect(box.open).toBe(false);
});
test('incoming trials show the authored direction',async()=>{
 const f=fixture();await f.view.ready;const row=all(f.root).find(x=>x.className==='k-row');row.listeners.click();
 const head=all(f.root).find(x=>x.className==='k-trial-head');expect(text(head)).toMatch(/child\s+→\s+lion/);
});
test('concurrent discoveries survive a conflict',async()=>{
 const f=fixture();await f.view.ready;const remote=f.stored();remote.known.push('child');remote.found.push({link:'child-lion',level:'light',hinted:false});remote.hypotheses.push({from:'child',to:'lion',text:'Remote',at:'now'});f.remote(remote);
 all(f.root).find(x=>x.className==='k-level'&&x.textContent==='Medium').listeners.click();
 await vi.waitFor(()=>expect(f.stored().level).toBe('medium'));expect(f.stored().known).toContain('child');expect(f.stored().found).toHaveLength(1);expect(f.stored().hypotheses).toHaveLength(1);
});
test('assistance changes from the interlocutor update the map',async()=>{
 const f=fixture();await f.view.ready;expect(text(f.root)).toContain('Questions at the threshold');
 f.view.setHelpLevel('heavy');expect(text(f.root)).not.toContain('Questions at the threshold');
 await vi.waitFor(()=>expect(f.stored().level).toBe('heavy'));
});
