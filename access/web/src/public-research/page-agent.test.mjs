import {afterEach,expect,test,vi} from 'vitest';
vi.mock('./research-history.mjs',()=>({loadResearchVersion:vi.fn()}));
vi.mock('./operation-view.mjs',()=>({mountResearchOperations:vi.fn()}));
vi.mock('../webmcp.ts',async importOriginal=>({...await importOriginal(),createWebMCPAdapter:vi.fn(()=>({start:vi.fn(),stop:vi.fn(),refresh:vi.fn()}))}));
import {loadResearchVersion} from './research-history.mjs';
import {createWebMCPAdapter} from '../webmcp.ts';
import {createResearchIndex,citationId} from './research-index.mjs';
import {mountResearchExperience} from './experience-view.mjs';
import {mountCognitiveSession} from './cognitive-session.mjs';
import {connectResearchPage} from './page-agent.mjs';
import '../observatory/human-forms-wasm-test-runtime.mjs';

class Element{
  constructor(tag){this.tagName=tag;this.children=[];this.listeners={};this.attributes={};this.dataset={};this.value='';this.textContent='';this.isConnected=true;this.classList={add(){},remove(){},toggle(){}};}
  append(...items){this.children.push(...items);}
  insertBefore(item,other){const at=this.children.indexOf(other);this.children.splice(at<0?this.children.length:at,0,item);}
  replaceChildren(...items){this.children=[...items];}
  setAttribute(key,value){this.attributes[key]=value;}
  addEventListener(key,fn){this.listeners[key]=fn;}
  removeEventListener(){}
  remove(){}
  get childElementCount(){return this.children.length;}
}
const all=root=>[root,...root.children.flatMap(all)];
const current='c'.repeat(64),old='a'.repeat(64),missing='b'.repeat(64),active=[];
afterEach(()=>{for(const destroy of active.splice(0))destroy();vi.unstubAllGlobals();vi.clearAllMocks();});

function fixture({work='also-sprach-zarathustra'}={}){
  vi.stubGlobal('document',{createElement:tag=>new Element(tag)});
  vi.stubGlobal('location',{href:`http://localhost/?experience=cognitive&item=same&revision=${old}`});
  const history={state:null};for(const method of ['replaceState','pushState'])history[method]=vi.fn((_state,_title,url)=>{location.href=String(url);});
  vi.stubGlobal('window',{history,addEventListener(){},removeEventListener(){},scrollTo(){},scrollY:0});vi.stubGlobal('requestAnimationFrame',fn=>fn());
  const cites={old:{w:work,c:'I.01',u:['old-unit','old-unit']},current:{w:work,c:'I.01',u:['new-unit','new-unit']}};
  const core=which=>({concepts:[{id:'same',title:{en:which==='old'?'Earlier meaning':'Current meaning'},essence:{en:which==='old'?'Earlier grounds':'Current grounds',cite:[cites[which]]}}],figures:[],proposed:[],chains:[],links:[]});
  loadResearchVersion.mockImplementation(async revision=>revision===old?{status:'available',revision,core:core('old')}:{status:'version-unavailable',revision});
  const index=createResearchIndex({nodes:[],space:{core:core('current')}},{revision:current});
  const saved=new Map(),copy=value=>value===undefined?undefined:structuredClone(value);
  const notebook={getPreference:async key=>copy(saved.get(key)),setPreference:async(key,value,options)=>{if(options&&JSON.stringify(options.expectedValue)!==JSON.stringify(saved.get(key)))throw Object.assign(new Error('conflict'),{code:'conflict'});saved.set(key,copy(value));},exportPacket:async()=>({preferences:[...saved].map(([key,value])=>({key,value:copy(value)}))})};
  const source={workId:'work',expressionId:'expression',editionId:'edition',itemId:'item',fileId:'file',fileSha256:'d'.repeat(64),textLayerRef:'witness.txt',textLayerSha256:'e'.repeat(64)},units={'old-unit':'Earlier witness words','new-unit':'Current witness words'};
  const fragments={fragment:vi.fn(()=>new Element('cite')),render:vi.fn(),sourceSelection:vi.fn(async cite=>({documentId:'work',versionId:'witness',revision:source.textLayerSha256,source,selections:cite.u.slice(0,1).map(unitId=>({unitId,raw:units[unitId],start:0,end:units[unitId].length}))}))};
  const provider={window:vi.fn(async args=>({...args,units:[{id:args.unitId,text:units[args.unitId]}]}))};
  const api={profile:'test',provider,getQuestion:()=> 'Why?',ensureCurrentQuestion:async()=>true};
  const root=new Element('root'),space={root:new Element('space'),fragments,selection:()=>null,renderResearch:vi.fn(()=>true),openResearch:vi.fn()};root.append(space.root);
  const experience=mountResearchExperience({root,header:new Element('header'),profile:'test',locale:()=> 'en',index,space,notebook,api});
  const onOpen=vi.fn((id,options)=>experience.open(id,options));
  const session=mountCognitiveSession({root:experience.cognitiveRoot,notebook,index,api,locale:()=> 'en',profile:'test',getSelection:()=>experience.getState(),versionContext:experience.versionContext,onOpen});
  const page=connectResearchPage({index,space,experience,session,api,workspace:{summary:()=>({revision:0})},locale:()=> 'en'});
  const tools=createWebMCPAdapter.mock.calls.at(-1)[3];
  const invoke=async(name,input={})=>JSON.parse((await tools.find(tool=>tool.name===name).execute(input)).content[0].text);
  active.push(()=>{page.destroy();session.destroy();experience.destroy();});
  return {index,experience,session,page,invoke,cites,saved,onOpen,root,fragments,provider};
}

test('agent reads share the retained UI resolver and preserve citation dependency IDs',async()=>{
  const f=fixture();await f.session.ready;
  const historic=await f.invoke('tos.bonsai.research-item',{id:'same'}),present=await f.invoke('tos.bonsai.research-item',{id:'same',revision:current});
  expect(historic.value).toMatchObject({id:'same',revision:old,title:{en:'Earlier meaning'},record:{essence:{en:'Earlier grounds'}}});
  expect(present.value).toMatchObject({id:'same',revision:current,title:{en:'Current meaning'},record:{essence:{en:'Current grounds'}}});
  expect(f.page.registry.context()).toMatchObject({index_revision:old,selected:{label:'Earlier meaning',source_refs:[citationId(f.cites.old)]}});
  expect((await f.page.registry.invoke('tos.page.inspect-selection')).value.revision).toBe(old);
  expect((await f.invoke('tos.bonsai.session-context')).value.context).toMatchObject({index_revision:old,selected:{title:{en:'Earlier meaning'}}});
  expect((await f.invoke('tos.bonsai.session-context',{revision:current})).value.context).toMatchObject({index_revision:current,selected:{title:{en:'Current meaning'}}});
  expect(loadResearchVersion).toHaveBeenCalledTimes(1);
  await f.page.registry.invoke('tos.page.select',{id:'same',revision:current});
  expect(f.experience.getState().revision).toBe(current);
  expect((await f.invoke('tos.bonsai.research-item',{id:'same'})).value.record.essence.en).toBe('Current grounds');
});

test('source reads use the citation record from the exact selected or explicit revision',async()=>{
  const f=fixture();await f.session.ready;
  const historic=await f.invoke('tos.bonsai.read-source',{item_id:citationId(f.cites.old)});
  expect(historic.value).toMatchObject({index_revision:old,units:[{text:'Earlier witness words',selection:{text:'Earlier witness words'}}]});
  expect(f.fragments.sourceSelection).toHaveBeenLastCalledWith(f.cites.old,expect.anything());
  await expect(f.invoke('tos.bonsai.read-source',{item_id:citationId(f.cites.current)})).rejects.toThrow('exact passage ID');
  const present=await f.invoke('tos.bonsai.read-source',{item_id:citationId(f.cites.current),revision:current});
  expect(present.value).toMatchObject({index_revision:current,units:[{text:'Current witness words'}]});
  expect(present.value.units[0].exactReference.target.textLayerSha256).toBe('e'.repeat(64));
});

test('finishing a retained index load refreshes tools without cancelling the pending selection',async()=>{
  const f=fixture(),load=loadResearchVersion.getMockImplementation();let release;
  const gate=new Promise(resolve=>{release=resolve;});loadResearchVersion.mockImplementation(async revision=>{await gate;return load(revision);});
  await f.session.ready;expect(f.page.registry.context().selected).toBeNull();
  const selecting=f.page.registry.invoke('tos.page.select',{id:'same'});release();
  expect((await selecting).value).toMatchObject({id:'same',revision:old,label:'Earlier meaning'});
  expect(f.page.adapter.refresh).toHaveBeenCalled();
});

test('a retained card read action opens its citation in the same retained version',async()=>{
  const f=fixture({work:'another-work'});await f.session.ready;
  f.root.listeners['public-research-action']({detail:{action:'read',cite:f.cites.old,revision:old}});
  await vi.waitFor(()=>expect(f.experience.getState()).toMatchObject({mode:'reference',id:citationId(f.cites.old),revision:old}));
  expect(f.index.get(citationId(f.cites.old))).toBeNull();
});

test('queued events, response sources and history transitions bind their event index',async()=>{
  const f=fixture();await f.session.ready;
  const pending=f.session.record('thought',{text:'An earlier reading'});f.experience.open('same',{revision:current});
  const event=await pending;expect(event.index_revision).toBe(old);
  const next=await f.session.dispatch('next-event',{timeout_ms:0});
  expect(next).toMatchObject({event:{index_revision:old},context:{index_revision:old,selected:{record:{essence:{en:'Earlier grounds'}}}}});
  const response={event_id:event.id,question_revision:event.question_revision,index_revision:event.index_revision,body:'The retained grounds',source_ids:[citationId(f.cites.current)]};
  await expect(f.session.dispatch('respond',response)).rejects.toThrow('event research index');
  const result=await f.session.dispatch('respond',{...response,source_ids:[citationId(f.cites.old)]});
  expect(result.response).toMatchObject({index_revision:old,stale:true});
  expect((await f.session.context()).experience.disclosed_source_ids).toEqual([]);
  expect((await f.session.context({revision:old})).experience.disclosed_source_ids).toEqual([citationId(f.cites.old)]);
  await vi.waitFor(()=>expect(all(f.root).some(item=>item.tagName==='button'&&item.textContent==='Earlier meaning')).toBe(true));
  all(f.root).find(item=>item.tagName==='button'&&item.textContent==='Earlier meaning').listeners.click();
  expect(f.onOpen).toHaveBeenLastCalledWith('same',{revision:old});expect(f.experience.getState().revision).toBe(old);
  const sourceButton=all(f.root).find(item=>item.className==='cognitive-response').children.find(item=>item.tagName==='button');sourceButton.listeners.click();
  expect(f.onOpen).toHaveBeenLastCalledWith(citationId(f.cites.old),{revision:old});
  const fresh=await f.session.record('research',{id:'same'}),delivered=await f.session.dispatch('respond',{...response,event_id:fresh.id,question_revision:fresh.question_revision,index_revision:fresh.index_revision,source_ids:[citationId(f.cites.old)]});
  expect(delivered.response.stale).toBe(false);expect((await f.session.context()).last_ack).toBe(fresh.id);
});

test('an unavailable selected version fails explicitly without returning or recording current content',async()=>{
  const f=fixture();await f.session.ready;f.experience.open('same',{revision:missing});
  expect(f.page.registry.context()).toMatchObject({index_revision:missing,selected:null});
  for(const [tool,input]of [['tos.bonsai.research-item',{id:'same'}],['tos.bonsai.read-source',{item_id:citationId(f.cites.current)}],['tos.bonsai.session-context',{}]])await expect(f.invoke(tool,input)).rejects.toThrow('version is unavailable');
  await expect(f.session.record('research')).rejects.toThrow('version is unavailable');
  expect([...f.saved.values()].some(value=>value?.schema==='tos_cognitive_event_v1')).toBe(false);
  expect(f.provider.window).not.toHaveBeenCalled();expect(f.fragments.sourceSelection).not.toHaveBeenCalled();
  expect((await f.invoke('tos.bonsai.research-item',{id:'same',revision:current})).value.title.en).toBe('Current meaning');
  expect(f.experience.getState().revision).toBe(missing);
});
