import {afterEach,expect,test,vi} from 'vitest';
import {mountKnowing} from './knowing-map.mjs';
class Element{
 constructor(tag){this.tagName=tag;this.children=[];this.listeners={};this.attributes={};this.dataset={};this.style={setProperty(){}};this.classes=new Set();this.classList={add:(...xs)=>xs.forEach(x=>this.classes.add(x)),remove:(...xs)=>xs.forEach(x=>this.classes.delete(x)),contains:x=>this.classes.has(x),toggle:(x,v)=>v?this.classes.add(x):this.classes.delete(x)};this.textContent='';}
 set className(value){this._className=value;for(const x of value.split(' '))this.classes.add(x);} get className(){return this._className;}
 focus(){} append(...items){this.children.push(...items);} replaceChildren(...items){this.children=[...items];} setAttribute(k,v){this.attributes[k]=v;if(k==='class')this.className=v;} addEventListener(k,fn){this.listeners[k]=fn;} querySelector(sel){return all(this).find(x=>x.classes.has(sel.slice(1)))??null;} get childElementCount(){return this.children.length;} get firstChild(){return this.children[0];}
}
const all=node=>[node,...node.children.flatMap(all)];const text=node=>all(node).map(n=>n.textContent).join(' ');
const live=[];afterEach(()=>{live.splice(0).forEach(x=>x.destroy());vi.unstubAllGlobals();});
function fixture({getSession}={}){
 vi.stubGlobal('document',{createElement:t=>new Element(t),createElementNS:(_,t)=>new Element(t)});vi.stubGlobal('ResizeObserver',class{observe(){}});vi.stubGlobal('requestAnimationFrame',()=>0);
 const nodes=['lion','child','other'].map(id=>({id,owner:'bonsai/core',kind:'figure',title:{en:id},record:{title:{en:id}}}));
 const link={id:'child-lion',kind:'relation',record:{admission:'map',from:'child',to:'lion',verb:{en:'becomes'},question:{en:'How?'},grounds:[]}};
 const second={id:'other-lion',kind:'relation',record:{admission:'map',from:'other',to:'lion',verb:{en:'asks'},grounds:[]}};
 let stored={level:'light',start:'lion',known:['lion'],found:[],hypotheses:[]};const copy=x=>structuredClone(x);
 const notebook={getPreference:async()=>copy(stored),setPreference:async(_,value,{expectedValue})=>{if(JSON.stringify(stored)!==JSON.stringify(expectedValue))throw Object.assign(Error('conflict'),{code:'conflict'});stored=copy(value);}};
 const index={items:new Map([...nodes,link,second].map(x=>[x.id,x]))},changes=[];
 const mount=()=>{const root=new Element('div'),view=mountKnowing({root,index,locale:()=> 'en',notebook,profile:'test',fragments:{fragment:()=>new Element('p')},getSession,onChange:value=>changes.push(value)});live.push(view);return {root,view};};
 const {root,view}=mount();
 return {root,view,mount,changes,stored:()=>copy(stored),remote:value=>stored=copy(value)};
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
test('a revealed link keeps the reader’s words until the interlocutor records them',async()=>{
 let answer=null;const record=vi.fn(async()=>answer);const f=fixture({getSession:()=>({record})});await f.view.ready;
 all(f.root).find(x=>x.className==='k-row').listeners.click();
 all(f.root).find(x=>x.tagName==='button'&&x.textContent==='Show the book’s answer').listeners.click();
 const area=all(f.root).find(x=>x.tagName==='textarea'),send=all(f.root).find(x=>x.tagName==='button'&&x.textContent==='Send to the interlocutor');
 await vi.waitFor(()=>expect(f.stored().found).toHaveLength(1));await new Promise(resolve=>setTimeout(resolve,0));
 area.value='The child is the lion’s second birth.';send.listeners.click();
 await vi.waitFor(()=>expect(record).toHaveBeenCalledWith('thought',{id:'child-lion',text:'The child is the lion’s second birth.'}));
 await vi.waitFor(()=>expect(send.disabled).toBe(false));expect(area.value).toBe('The child is the lion’s second birth.');expect(text(f.root)).toContain('not taking thoughts now');
 answer={id:'event-1'};send.listeners.click();
 await vi.waitFor(()=>expect(area.value).toBe(''));expect(text(f.root)).toContain('Sent.');
});
test('a heavy-mode reading reaches the interlocutor with the opened link',async()=>{
 const record=vi.fn(async()=>({id:'event'}));const f=fixture({getSession:()=>({record})});await f.view.ready;f.view.setHelpLevel('heavy');
 const star=id=>all(f.root).find(x=>x.attributes['data-id']===id);
 star('lion').listeners.click({stopPropagation(){}});star('child').listeners.click({stopPropagation(){}});
 const area=all(f.root).find(x=>x.tagName==='textarea');area.value='The child is a new beginning.';
 all(f.root).find(x=>x.tagName==='button'&&x.textContent==='Check against the book').listeners.click();
 await vi.waitFor(()=>expect(record).toHaveBeenCalledWith('trial',expect.objectContaining({id:'child-lion',text:expect.stringContaining('The reader’s reading: The child is a new beginning.')})));
});

test('a return offers the threshold where the person stopped, else the nearest one',async()=>{
 const f=fixture();await f.view.ready;
 expect(text(f.root)).toContain('The nearest threshold');
 const row=all(f.root).filter(x=>x.className==='k-row').find(x=>text(x).includes('?'));
 // The second threshold has no question of its own, nor has its star: it says what is to be joined.
 expect(text(f.root)).toContain('To be joined: “lion” and a neighbouring star, in the book’s word.');
 all(f.root).filter(x=>x.className==='k-row').at(-1).listeners.click();
 await vi.waitFor(()=>expect(f.stored().last?.link).toBeTruthy());
 const stopped=f.stored().last.link;
 const again=f.mount();await again.view.ready;
 expect(text(again.root)).toContain('You stopped here');
 all(again.root).find(x=>x.textContent==='Continue from the threshold').listeners.click();
 expect(text(all(again.root).find(x=>x.className==='k-trial-head'))).toContain(stopped==='other-lion'?'other':'child');
 expect(row).toBeTruthy();
});
test('what is opened reaches the original sky as its own layer',async()=>{
 const f=fixture();await f.view.ready;
 expect(f.changes.at(-1)).toEqual({known:['lion'],found:[]});
});
