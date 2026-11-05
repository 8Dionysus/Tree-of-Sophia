import {afterEach,expect,test,vi} from 'vitest';
vi.mock('./research-history.mjs',()=>({loadResearchVersion:vi.fn(async revision=>({status:'version-unavailable',revision}))}));
vi.mock('./operation-view.mjs',()=>({mountResearchOperations:vi.fn(()=>({destroy(){},refreshLocale(){}}))}));
import {claimLine,mountResearchExperience} from './experience-view.mjs';
import {mountResearchOperations} from './operation-view.mjs';

class Element{
  constructor(tag){this.tag=tag;this.children=[];this.listeners={};this.dataset={};this.attributes={};this.isConnected=true;this.classList={add(){},remove(){},toggle(){}};this.textContent='';}
  append(...items){this.children.push(...items);}
  insertBefore(item,other){const at=this.children.indexOf(other);this.children.splice(at<0?this.children.length:at,0,item);}
  replaceChildren(...items){this.children=[...items];}
  setAttribute(key,value){this.attributes[key]=value;}
  addEventListener(key,fn){this.listeners[key]=fn;}
  removeEventListener(){}
  remove(){}
}
afterEach(()=>vi.unstubAllGlobals());

test('reference navigation keeps an unavailable historic version distinct from the current item',async()=>{
  const document={createElement:tag=>new Element(tag)};
  const window={history:{replaceState:vi.fn(),pushState:vi.fn(),state:null},addEventListener(){},removeEventListener(){},scrollTo(){},scrollY:0};
  vi.stubGlobal('document',document);vi.stubGlobal('window',window);vi.stubGlobal('location',{href:'http://localhost/?experience=reference&item=one&revision=old'});vi.stubGlobal('requestAnimationFrame',fn=>fn());
  const entries=new Map([['one',{id:'one',kind:'conception',title:{en:'One'},dependencies:[]}]]),index={revision:'current',items:entries,get:id=>entries.get(id),resolveId:id=>id,search:()=>[],references:()=>[]};
  const root=new Element('root'),spaceroot=new Element('space'),space={root:spaceroot,renderResearch:vi.fn(),openResearch:vi.fn(),fragments:{},selection:()=>null};root.append(spaceroot);
  let stored;const notebook={getPreference:vi.fn(async()=>undefined),setPreference:vi.fn(async(_key,value)=>{stored=value;})};
  const header=new Element('header'),view=mountResearchExperience({root,header,profile:'test',locale:()=> 'en',index,space,notebook,api:{getQuestion:()=>''}});
  await vi.waitFor(()=>expect(root.children.some(node=>node.className==='experience-pane'&&node.children.some(child=>child.className==='experience-body'&&child.children.find(item=>item.className==='experience-article')?.children.some(item=>item.textContent?.includes('version is unavailable'))))).toBe(true));
  expect(view.getState()).toMatchObject({mode:'reference',id:'one',revision:'old'});expect(space.renderResearch).not.toHaveBeenCalled();
  // The modes and tools live in the page's one bar, the header, in every mode.
  expect(header.children.some(node=>node.className==='experience-toolbar')).toBe(true);
  const find=(node,test)=>test(node)?node:node.children?.map(child=>find(child,test)).find(Boolean);
  find(header,node=>node.tag==='button'&&node.textContent==='Calculations').listeners.click();
  await vi.waitFor(()=>expect(root.children.find(node=>node.className==='experience-pane').children.at(-1).children[1].textContent).toContain('version is unavailable'));
  expect(mountResearchOperations).not.toHaveBeenCalled();
  view.setMode('original');expect(space.openResearch).not.toHaveBeenCalled();view.setMode('reference');
  view.open('one');await vi.waitFor(()=>expect(stored?.revision).toBe('current'));expect(space.renderResearch).toHaveBeenCalled();
  expect(mountResearchOperations).toHaveBeenCalledWith(expect.objectContaining({index}));
  expect(stored.stack.map(entry=>entry.revision)).toEqual(['old','current']);view.setMode('original');expect(space.openResearch).toHaveBeenCalledWith('one');view.destroy();
});

test('a concurrent preference change preserves the local draft before recovery',async()=>{
  const window={history:{replaceState:vi.fn(),pushState:vi.fn(),state:null},addEventListener(){},removeEventListener(){},scrollTo(){},scrollY:0};
  vi.stubGlobal('document',{createElement:tag=>new Element(tag)});vi.stubGlobal('window',window);vi.stubGlobal('location',{href:'http://localhost/'});vi.stubGlobal('requestAnimationFrame',fn=>fn());
  const index={revision:'current',items:new Map(),get:()=>null,resolveId:id=>id,search:()=>[],references:()=>[]},root=new Element('root'),space={root:new Element('space'),renderResearch:vi.fn(),selection:()=>null,fragments:{}};root.append(space.root);
  let remote;const writes=[];const notebook={getPreference:vi.fn(async()=>remote),setPreference:vi.fn(async(key,value,options)=>{writes.push({key,value});if(options&&JSON.stringify(options.expectedValue)!==JSON.stringify(remote))throw Object.assign(new Error('conflict'),{code:'conflict'});if(!key.includes('.recovery.'))remote=value;})};
  const view=mountResearchExperience({root,header:new Element('header'),profile:'test',locale:()=> 'en',index,space,notebook,api:{getQuestion:()=>''}});
  await vi.waitFor(()=>expect(notebook.getPreference).toHaveBeenCalled());remote={mode:'original',id:null,revision:'current',stack:[],cursor:-1,pins:[],question:'remote'};
  view.setMode('reference');const pane=root.children.find(child=>child.className==='experience-pane'),recover=pane.children.find(child=>child.tag==='button');
  await vi.waitFor(()=>expect(recover.hidden).toBe(false));recover.listeners.click();
  await vi.waitFor(()=>expect(writes.some(item=>item.key.includes('.recovery.')&&item.value.mode==='reference')).toBe(true));
  await vi.waitFor(()=>expect(view.getState().mode).toBe('original'));expect(remote.question).toBe('remote');view.destroy();
});

test('refreshing an unchanged question preserves the notebook generation used by import preview',async()=>{
  const window={history:{replaceState:vi.fn(),pushState:vi.fn(),state:null},addEventListener(){},removeEventListener(){},scrollTo(){},scrollY:0};
  vi.stubGlobal('document',{createElement:tag=>new Element(tag)});vi.stubGlobal('window',window);vi.stubGlobal('location',{href:'http://localhost/'});vi.stubGlobal('requestAnimationFrame',fn=>fn());
  const index={revision:'current',items:new Map(),get:()=>null,search:()=>[],references:()=>[]},root=new Element('root'),space={root:new Element('space'),selection:()=>null,fragments:{}};root.append(space.root);
  const notebook={getPreference:vi.fn(async()=>undefined),setPreference:vi.fn(async()=>{})};
  let value='',refresh;
  const api={getQuestion:()=>value,subscribeQuestion:listener=>{refresh=listener;return()=>{};}};
  const view=mountResearchExperience({root,header:new Element('header'),profile:'test',locale:()=> 'en',index,space,notebook,api});
  await vi.waitFor(()=>expect(notebook.getPreference).toHaveBeenCalled());
  refresh();refresh();await Promise.resolve();expect(notebook.setPreference).not.toHaveBeenCalled();
  value='Why does the receiver matter?';refresh();refresh();
  await vi.waitFor(()=>expect(notebook.setPreference).toHaveBeenCalledTimes(1));
  expect(view.getState().question).toBe(value);view.destroy();
});

test('moving through history keeps the scroll position each entry had',async()=>{
  const window={history:{replaceState:vi.fn(),pushState:vi.fn(),state:null},addEventListener(){},removeEventListener(){},scrollTo:vi.fn(),scrollY:0};
  vi.stubGlobal('document',{createElement:tag=>new Element(tag)});vi.stubGlobal('window',window);vi.stubGlobal('location',{href:'http://localhost/?experience=reference'});vi.stubGlobal('requestAnimationFrame',fn=>fn());
  const entries=new Map(['a','b'].map(id=>[id,{id,kind:'conception',title:{en:id.toUpperCase()},dependencies:[]}])),index={revision:'current',items:entries,get:id=>entries.get(id),resolveId:id=>id,search:()=>[],references:()=>[]};
  const root=new Element('root'),space={root:new Element('space'),renderResearch:vi.fn(),openResearch:vi.fn(),fragments:{},selection:()=>null};root.append(space.root);
  const notebook={getPreference:vi.fn(async()=>undefined),setPreference:vi.fn(async()=>{})};
  const header=new Element('header'),view=mountResearchExperience({root,header,profile:'test',locale:()=> 'en',index,space,notebook,api:{getQuestion:()=>''}});
  await vi.waitFor(()=>expect(notebook.getPreference).toHaveBeenCalled());await new Promise(resolve=>setTimeout(resolve,0));
  view.open('a');window.scrollY=100;view.open('b');window.scrollY=500;
  const all=node=>[node,...(node.children??[]).flatMap(all)],back=all(header).find(node=>node.tag==='button'&&node.title==='Back');
  back.listeners.click();
  expect(window.scrollTo).toHaveBeenLastCalledWith(0,100);expect(view.getState().stack.map(entry=>entry.scroll)).toEqual([100,500]);
  view.destroy();
});

test('a stand without a sky of its own offers no learning mode, and an address that asks for it opens the space',async()=>{
  const window={history:{replaceState:vi.fn(),pushState:vi.fn(),state:null},addEventListener(){},removeEventListener(){},scrollTo(){},scrollY:0};
  vi.stubGlobal('document',{createElement:tag=>new Element(tag)});vi.stubGlobal('window',window);vi.stubGlobal('location',{href:'http://localhost/?experience=cognitive'});vi.stubGlobal('requestAnimationFrame',fn=>fn());
  const index={revision:'current',items:new Map(),get:()=>null,resolveId:id=>id,search:()=>[],references:()=>[]},root=new Element('root'),space={root:new Element('space'),renderResearch:vi.fn(),selection:()=>null,fragments:{}};root.append(space.root);
  const notebook={getPreference:vi.fn(async()=>undefined),setPreference:vi.fn(async()=>{})},header=new Element('header');
  const view=mountResearchExperience({root,header,profile:'test',locale:()=> 'en',index,space,notebook,learning:false,api:{getQuestion:()=>''}});
  const modes=[];const walk=node=>{if(node?.tag==='button'&&node.dataset?.mode)modes.push(node.dataset.mode);for(const child of node?.children??[])walk(child);};walk(header);
  expect(modes).toEqual(['original','reference']);
  expect(view.getState().mode).toBe('original');
  view.setMode('cognitive');expect(view.getState().mode).toBe('original');
  view.setMode('reference');expect(view.getState().mode).toBe('reference');view.destroy();
});

test('a long claim is named by whole sentences, a single long sentence by whole words',()=>{
  const short='Литания истолковывает I-A как вечное «да».';
  expect(claimLine(short)).toBe(short);
  const two=`${'Первое предложение говорит о литании и о вечном «да» осла, которое славят высшие люди. '.repeat(2)}Третье предложение продолжает мысль о празднике и о согласии на жизнь.`;
  expect(claimLine(two)).toMatch(/осла, которое славят высшие люди\.$/u);
  const one='Заратустра встречает его без унижающей милостыни и даёт убежище после общего праздника и говорит что один день примирил его с землёй и дорогой к новому утру и вечному возвращению всего что было и будет снова и снова под тем же небом';
  const line=claimLine(one);
  expect(line.endsWith('…')).toBe(true);expect(one.startsWith(line.slice(0,-1))).toBe(true);expect(one[line.length-1]).toBe(' ');
});

test('the space is given the reader’s trail of cards and walks it back and forth',async()=>{
  const window={history:{replaceState:vi.fn(),pushState:vi.fn(),state:null},addEventListener(){},removeEventListener(){},scrollTo:vi.fn(),scrollY:0};
  vi.stubGlobal('document',{createElement:tag=>new Element(tag)});vi.stubGlobal('window',window);vi.stubGlobal('location',{href:'http://localhost/?experience=original'});vi.stubGlobal('requestAnimationFrame',fn=>fn());
  const entries=new Map(['a','b','c'].map(id=>[id,{id,kind:'conception',title:{en:id.toUpperCase()},dependencies:[]}])),index={revision:'current',items:entries,get:id=>entries.get(id),resolveId:id=>id,search:()=>[],references:()=>[]};
  const root=new Element('root'),space={root:new Element('space'),renderResearch:vi.fn(),openResearch:vi.fn(),setTrail:vi.fn(),fragments:{},selection:()=>null};root.append(space.root);
  const notebook={getPreference:vi.fn(async()=>undefined),setPreference:vi.fn(async()=>{})};
  const view=mountResearchExperience({root,header:new Element('header'),profile:'test',locale:()=> 'en',index,space,notebook,api:{getQuestion:()=>''}});
  await vi.waitFor(()=>expect(notebook.getPreference).toHaveBeenCalled());await new Promise(resolve=>setTimeout(resolve,0));
  view.open('a');view.open('b');view.open('c');
  const last=()=>space.setTrail.mock.calls.at(-1)[0];
  expect(last().items.map(item=>item.title)).toEqual(['A','B','C']);expect(last().cursor).toBe(2);
  last().go(-2);expect(last().cursor).toBe(0);expect(space.openResearch).toHaveBeenLastCalledWith('a');
  view.destroy();
});
