import {describe,it,expect} from 'vitest';
import {createConstructorModel} from './model.mjs';
import {carrySemanticWorkspace} from './semantic-workspace.mjs';
const locks={request:async(_name,fn)=>fn()};
const carry=(storage,lib=library,r=routes,b=binding)=>carrySemanticWorkspace(storage,lib,r,b,locks);
const from='a'.repeat(64),to='b'.repeat(64),binding={from,to};
const library={schema:'tos_constructor_library_v1',fingerprint:to,rootId:'work',nodes:[{id:'work',kind:'work',parentId:null,title:{ru:'Книга',en:'Book'}}]};
const routes=[{id:'walk',steps:[{},{}]}],treeKey=d=>'tos-living-tree-v1:'+d,routeKey=d=>'tos-reading-route-v1:'+d;
function fixture(){
 const model=createConstructorModel({...library,fingerprint:from},{storage:null});model.seed();model.addDraftWithContext({kind:'note',title:'My reading',body:'A question',position:[12,34,56]},{sourceId:'material:work'});model.rename('My tree');
 const original=model.exportPacket(),point=JSON.stringify({version:1,routeId:'walk',index:1,finished:true}),data=new Map([[treeKey(from),original],[routeKey(from),point]]);
 return {original,point,data,storage:{getItem:k=>data.get(k)??null,setItem:(k,v)=>data.set(k,v)}};
}
describe('carrying a personal workspace to a reviewed semantic edition',()=>{
 it('preserves all personal data, positions and route state, keeping the earlier bytes',async()=>{const f=fixture();expect(await carry(f.storage)).toEqual({copied:['tree','route'],errors:[]});expect(JSON.parse(f.data.get(treeKey(to)))).toEqual({...JSON.parse(f.original),libraryFingerprint:to});expect(f.data.get(treeKey(from))).toBe(f.original);expect(f.data.get(routeKey(from))).toBe(f.point);expect(f.data.get(routeKey(to))).toBe(f.point);});
 it('does not overwrite an existing current edition, including invalid current data',async()=>{const f=fixture();f.data.set(treeKey(to),'invalid but owned');f.data.set(routeKey(to),'');expect((await carry(f.storage)).copied).toEqual([]);expect(f.data.get(treeKey(to))).toBe('invalid but owned');expect(f.data.get(routeKey(to))).toBe('');});
 it('does nothing for an unreviewed edition or missing earlier data',async()=>{const f=fixture();expect(await carry(f.storage,{...library,fingerprint:'c'.repeat(64)})).toEqual({copied:[],errors:[]});expect(f.data.has(treeKey(to))).toBe(false);expect(await carry({getItem:()=>null})).toEqual({copied:[],errors:[]});});
 it('rejects invalid personal references and route positions before copying them',async()=>{const f=fixture(),bad=JSON.parse(f.original);bad.nodes[0].materialId='unknown';f.data.set(treeKey(from),JSON.stringify(bad));f.data.set(routeKey(from),JSON.stringify({version:1,routeId:'walk',index:8,finished:false}));const result=await carry(f.storage);expect(result.copied).toEqual([]);expect(result.errors.map(e=>e.kind)).toEqual(['tree','route']);expect(f.data.has(treeKey(to))).toBe(false);expect(f.data.has(routeKey(to))).toBe(false);});
 it('reports write failure while keeping the original data intact',async()=>{const f=fixture();f.storage.setItem=()=>{throw Error('Quota full');};const result=await carry(f.storage);expect(result.errors).toHaveLength(2);expect(f.data.get(treeKey(from))).toBe(f.original);expect(f.data.has(treeKey(to))).toBe(false);});
 it('preserves a deliberately cleared old tree and requires its exact old identity',async()=>{const f=fixture(),old=JSON.parse(f.original);old.nodes=[];old.edges=[];f.data.set(treeKey(from),JSON.stringify(old));await carry(f.storage);expect(JSON.parse(f.data.get(treeKey(to))).nodes).toEqual([]);const g=fixture();old.libraryFingerprint='c'.repeat(64);g.data.set(treeKey(from),JSON.stringify(old));expect((await carry(g.storage)).errors[0].kind).toBe('tree');expect(g.data.has(treeKey(to))).toBe(false);});
 it('rechecks a target created while the earlier packet is being read',async()=>{const f=fixture(),get=f.storage.getItem;f.storage.getItem=k=>{if(k===treeKey(from))f.data.set(treeKey(to),'New work');return get(k);};expect((await carry(f.storage)).copied).toEqual(['route']);expect(f.data.get(treeKey(to))).toBe('New work');});
 it('serializes simultaneous starts and refuses copying without coordination',async()=>{const f=fixture();let queue=Promise.resolve();const coordinated={request:(_name,fn)=>{queue=queue.then(fn);return queue;}};const results=await Promise.all([carrySemanticWorkspace(f.storage,library,routes,binding,coordinated),carrySemanticWorkspace(f.storage,library,routes,binding,coordinated)]);expect(results.map(r=>r.copied)).toEqual([['tree','route'],[]]);const g=fixture();expect((await carrySemanticWorkspace(g.storage,library,routes,binding,null)).errors).toHaveLength(2);expect(g.data.has(treeKey(to))).toBe(false);expect(g.data.get(treeKey(from))).toBe(g.original);});
 it('uses the most recent present edition while supporting readers who skipped it',async()=>{
  const newer='c'.repeat(64),reviewed={from:[newer,from],to},f=fixture();
  const recent={...JSON.parse(f.original),libraryFingerprint:newer,title:'Newer personal title'};
  f.data.set(treeKey(newer),JSON.stringify(recent));await carry(f.storage,library,routes,reviewed);
  expect(JSON.parse(f.data.get(treeKey(to)))).toEqual({...recent,libraryFingerprint:to});expect(f.data.get(treeKey(from))).toBe(f.original);
  const skipped=fixture();await carry(skipped.storage,library,routes,reviewed);expect(JSON.parse(skipped.data.get(treeKey(to)))).toEqual({...JSON.parse(skipped.original),libraryFingerprint:to});
 });
 it('never falls back past present but invalid newer personal work',async()=>{
  const newer='c'.repeat(64),f=fixture();f.data.set(treeKey(newer),'damaged but newer');
  const result=await carry(f.storage,library,routes,{from:[newer,from],to});
  expect(result.errors.map(error=>error.kind)).toEqual(['tree']);expect(f.data.has(treeKey(to))).toBe(false);expect(f.data.get(treeKey(newer))).toBe('damaged but newer');
 });
});
