import '../observatory/human-forms-wasm-test-runtime.mjs';
import {createConstructorModel} from '../../constructor/model.mjs';
import {createFragments} from './fragments.mjs';
import {describe,it,expect} from 'vitest';
import field from '../../../../bonsai/aesthetics/field.json';
import catalog from '../../../../bonsai/aesthetics/library.json';
import chronology from '../../../../bonsai/aesthetics/chronology.json';
import {aestheticsTree,aestheticsSourceGroups} from './aesthetics-tree.mjs';
import {aestheticsTimeline} from './space-timeline.mjs';
import {jaccardDistance} from './space-proximity.mjs';
import {colonnade} from './space-gatherings.mjs';
import {makeAesthetics} from './aesthetics-library.mjs';
describe('Aesthetics field projection',()=>{
  const result=makeAesthetics({field,library:catalog,chronology}),byId=new Map(result.nodes.map(node=>[node.id,node]));
  it('shows each conception and every cited place on each of its area routes',()=>{
    for(const item of field.conceptions){const node=byId.get(`aesthetics:reading:${item.id}`);expect(node).toBeDefined();expect(node.sourceNodeIds).toHaveLength(item.formula.cite.length);for(const id of node.sourceNodeIds)expect(byId.get(id)?.kind).toBe('fragment');for(const area of item.areas)expect(result.atlas.routes.find(route=>route.id===area)?.steps.map(step=>step.nodeId)).toContain(node.id);}
  });
  it('names a cited place in the reader’s language',async()=>{const fragments=createFragments({profile:'aesthetics',locale:()=> 'ru',t:key=>key});expect(await fragments.placeLabel({w:'baruch-spinoza.ethics',loc:'III def3 p132'})).toBe('Часть III, определение 3, с. 132');});
  it('seeds the bounded saved graph',()=>{const model=createConstructorModel(result,{storage:null});try{model.seedAtlas();expect(model.getState().nodes.length).toBeGreaterThan(0);}finally{model.dispose();}});
  it('places distinct stars in their strata, with groups in the crown and works by life',()=>{
    const layout=aestheticsTree({library:result,field,catalog,chronology}),seen=new Set();
    for(const node of result.nodes){const p=layout.positions.get(node.id);expect(p?.every(Number.isFinite)).toBe(true);expect(seen.has(JSON.stringify(p)),node.id).toBe(false);seen.add(JSON.stringify(p));}
    for(const area of field.areas.filter(a=>byId.has(`aesthetics:route:${a.id}`))){const [x,y,z]=layout.positions.get(`aesthetics:route:${area.id}`),angle=(Math.atan2(x,z)+2*Math.PI)%(2*Math.PI);expect(y).toBeLessThan(-900);expect(Math.floor(angle/(Math.PI/2))).toBe('ABCD'.indexOf(area.group));}
    for(const concept of result.nodes.filter(n=>n.kind==='concept')){const [x,y,z]=layout.positions.get(concept.id);expect(Math.hypot(x,z)).toBeLessThan(200);expect(y).toBeLessThan(0);}
    const works=Object.entries(catalog.works).sort(([,a],[,b])=>Number(chronology.situations[chronology.lives[a.agents[0]]].dating.interval.start)-Number(chronology.situations[chronology.lives[b.agents[0]]].dating.interval.start));
    for(const [i,[id]] of works.entries()){const [x,y,z]=layout.positions.get(`aesthetics:catalog:${id}`);expect(y).toBeGreaterThan(0);const angle=(Math.atan2(x,z)+2*Math.PI)%(2*Math.PI);expect(Math.min(Math.abs(angle-2*Math.PI*i/works.length),Math.abs(angle-2*Math.PI*i/works.length-2*Math.PI))).toBeLessThan(1e-8);}
  });
  it('lays each source and reading beside its work',()=>{
    const groups=aestheticsSourceGroups({library:result,chronology}),view=colonnade({authors:groups});
    for(const item of field.conceptions){const work=groups.flatMap(a=>a.works).find(w=>w.id===`aesthetics:catalog:${item.work}`);expect(work.front).toContain(`aesthetics:reading:${item.id}`);for(const id of byId.get(`aesthetics:reading:${item.id}`).sourceNodeIds)expect(work.up).toContain(id);}
    for(const group of groups)for(const work of group.works)for(const id of [work.id,...work.up,...work.front])expect(view.positions.has(id)).toBe(true);
  });
  it('dates every reading on its work and groups the rows A–D',()=>{
    const time=aestheticsTimeline({field,library:result,chronology});expect(time.lanes.map(lane=>lane.id)).toEqual(['A','B','C','D']);for(const lane of time.labels('en').filter(label=>label.variant==='lane')){expect(lane.ref).toBeUndefined();expect(lane.hint).toBeUndefined();}
    for(const item of field.conceptions){const id=`aesthetics:reading:${item.id}`;expect(time.positions.has(id),id).toBe(true);expect(time.timeOf(id).composition).toBe(chronology.compositions.works[item.work]);expect(time.spanOf(id)).toBeTruthy();}
    const [a,b]=field.conceptions;expect(jaccardDistance(a.of,b.of)).toBeGreaterThanOrEqual(0);expect(jaccardDistance(a.of,a.of)).toBe(0);
  });
  it('holds only grounded areas, concepts and relations',()=>{
    const used=new Set(field.conceptions.flatMap(item=>item.of));
    for(const item of field.concepts)expect(byId.has(`aesthetics:catalog:${item.id}`)).toBe(used.has(item.id));
    for(const area of field.areas)expect(byId.has(`aesthetics:route:${area.id}`)).toBe(field.conceptions.some(item=>item.areas.includes(area.id)));
    for(const link of field.links){const shown=result.atlas.edges.some(edge=>edge.id===`aesthetics:relation:${link.id}`);expect(shown).toBe(used.has(link.from)&&used.has(link.to)&&!!link.grounds?.length&&link.grounds.every(id=>field.conceptions.some(item=>item.id===id)));}
  });
});
