import {afterEach,expect,test,vi} from 'vitest';
vi.mock('./research-history.mjs',()=>({loadResearchVersion:vi.fn(async revision=>({status:'version-unavailable',revision}))}));
vi.mock('./operation-view.mjs',()=>({mountResearchOperations:vi.fn(()=>({destroy(){},refreshLocale(){}}))}));
import {mountResearchExperience} from './experience-view.mjs';

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
  const view=mountResearchExperience({root,header:new Element('header'),profile:'test',locale:()=> 'en',index,space,notebook,api:{getQuestion:()=>''}});
  await vi.waitFor(()=>expect(root.children.some(node=>node.className==='experience-pane'&&node.children.some(child=>child.className==='experience-body'&&child.children[0]?.children.some(item=>item.textContent?.includes('version is unavailable'))))).toBe(true));
  expect(view.getState()).toMatchObject({mode:'reference',id:'one',revision:'old'});expect(space.renderResearch).not.toHaveBeenCalled();
  view.open('one');await vi.waitFor(()=>expect(stored?.revision).toBe('current'));expect(space.renderResearch).toHaveBeenCalled();
  expect(stored.stack.map(entry=>entry.revision)).toEqual(['old','current']);view.destroy();
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
