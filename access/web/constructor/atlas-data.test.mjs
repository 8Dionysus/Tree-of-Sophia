import {describe,it,expect} from 'vitest';
import {createAtlasLibrary} from './atlas-data.mjs';
import {NODE_CONTENT} from './atlas-content.mjs';
import {createConstructorModel} from './model.mjs';
import {bindResearchRoutes,routeGraphInput} from './journey-state.mjs';
const bi=text=>({ru:text,en:text});
const ids=['work','chapter-p3.r2','moment','chapter-p3.r13','all-things','same-life','dossier'];
function sourceLibrary(){
 return {schema:'tos_constructor_library_v1',rootId:'work',fingerprint:'a'.repeat(64),nodes:[...ids,'unlisted-source'].map(id=>({id,kind:id==='work'?'work':'fragment',parentId:id==='work'?null:'work',title:bi(id),body:bi('Original source body'),quote:bi('Bound source quote'),exact:{de:'Exact German bytes',ru:'Exact Russian bytes'},sourceRefs:[{label:'Source address',ref:'source:'+id}],speaker:bi('Source speaker')}))};
}
const bilingual=value=>['ru','en'].every(lang=>typeof value?.[lang]==='string'&&value[lang].trim());
describe('prepared meaning and exact source boundaries',()=>{
 it('adds reading depth without changing the input or source text and provenance',async()=>{
  const source=sourceLibrary(),before=structuredClone(source),library=await createAtlasLibrary(source);
  expect(source).toEqual(before);
  for(const original of source.nodes){const enriched=library.nodes.find(n=>n.id===original.id);for(const field of ['body','quote','exact','sourceRefs','speaker'])expect(enriched[field]).toEqual(original[field]);}
  for(const id of ids)expect(library.nodes.find(n=>n.id===id).atlasBody).toEqual(NODE_CONTENT[id].body);
  expect(library.nodes.find(n=>n.id==='unlisted-source')).toEqual(source.nodes.at(-1));
 });
 it('keeps every prepared material bilingual and covered by its authored reading',async()=>{
  const library=await createAtlasLibrary(sourceLibrary()),prepared=library.nodes.filter(n=>n.demo||ids.includes(n.id));
  expect(new Set(Object.keys(NODE_CONTENT))).toEqual(new Set(prepared.map(n=>n.id)));
  for(const node of prepared){expect(bilingual(node.atlasBody??node.body)).toBe(true);for(const perspective of node.perspectives??[]){expect(bilingual(perspective.title)).toBe(true);expect(bilingual(perspective.body)).toBe(true);}for(const field of ['question','example'])if(node[field])expect(bilingual(node[field])).toBe(true);expect(bilingual(node.inquiry.argument)).toBe(true);expect(node.inquiry.grounds.length).toBeGreaterThan(0);}
  for(const edge of library.atlas.edges){expect(bilingual(edge.inquiry.warrant)).toBe(true);expect(edge.inquiry.grounds.length).toBeGreaterThan(0);}
 });
 it('can start every prepared walk as an atomic graph operation with its exact transitions',async()=>{
  const library=await createAtlasLibrary(sourceLibrary()),routes=bindResearchRoutes(library.atlas.routes,library);
  for(const route of routes)for(const field of ['startingPoint','stakes','carryForward'])expect(bilingual(route.investigation[field])).toBe(true);
  for(const route of routes){const model=createConstructorModel(library,{storage:null});model.seedAtlas();model.growAtlas(routeGraphInput(route));const state=model.getState();for(const step of route.steps)expect(state.nodes.some(n=>n.id===step.graphNodeId)).toBe(true);for(const transition of route.transitions)expect(state.edges.some(e=>e.id===transition.graphEdgeId)).toBe(true);}
 });
});
