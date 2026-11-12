import '../observatory/human-forms-wasm-test-runtime.mjs';
import {createConstructorModel} from '../../constructor/model.mjs';
import {describe,it,expect} from 'vitest';
import field from '../../../../bonsai/aesthetics/field.json';
import catalog from '../../../../bonsai/aesthetics/library.json';
import chronology from '../../../../bonsai/aesthetics/chronology.json';
import {aestheticsTree} from './aesthetics-tree.mjs';
import {makeAesthetics} from './aesthetics-library.mjs';
describe('Aesthetics field projection',()=>{
  const result=makeAesthetics({field,library:catalog,chronology}),byId=new Map(result.nodes.map(node=>[node.id,node]));
  it('shows each conception and every cited place on each of its area routes',()=>{
    for(const item of field.conceptions){const node=byId.get(`aesthetics:reading:${item.id}`);expect(node).toBeDefined();expect(node.sourceNodeIds).toHaveLength(item.formula.cite.length);for(const id of node.sourceNodeIds)expect(byId.get(id)?.kind).toBe('fragment');for(const area of item.areas)expect(result.atlas.routes.find(route=>route.id===area)?.steps.map(step=>step.nodeId)).toContain(node.id);}
  });
  it('seeds the bounded saved graph',()=>{const model=createConstructorModel(result,{storage:null});try{model.seedAtlas();expect(model.getState().nodes.length).toBeGreaterThan(0);}finally{model.dispose();}});
  it('places every star on the tree',()=>{const layout=aestheticsTree({library:result,field,catalog,chronology});for(const node of result.nodes)expect(layout.positions.get(node.id)?.every(Number.isFinite)).toBe(true);});
  it('holds only grounded areas, concepts and relations',()=>{
    const used=new Set(field.conceptions.flatMap(item=>item.of));
    for(const item of field.concepts)expect(byId.has(`aesthetics:catalog:${item.id}`)).toBe(used.has(item.id));
    for(const area of field.areas)expect(byId.has(`aesthetics:route:${area.id}`)).toBe(field.conceptions.some(item=>item.areas.includes(area.id)));
    for(const link of field.links){const shown=result.atlas.edges.some(edge=>edge.id===`aesthetics:relation:${link.id}`);expect(shown).toBe(used.has(link.from)&&used.has(link.to)&&!!link.grounds?.length&&link.grounds.every(id=>field.conceptions.some(item=>item.id===id)));}
  });
});
