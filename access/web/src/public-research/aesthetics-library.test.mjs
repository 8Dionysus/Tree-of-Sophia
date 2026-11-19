import '../observatory/human-forms-wasm-test-runtime.mjs';
import {createConstructorModel} from '../../constructor/model.mjs';
import {createFragments} from './fragments.mjs';
import {describe,it,expect} from 'vitest';
import field from '../../../../bonsai/aesthetics/field.json';
import catalog from '../../../../bonsai/aesthetics/library.json';
import chronology from '../../../../bonsai/aesthetics/chronology.json';
import places from '../../../../bonsai/aesthetics/fragments.json';
import {aestheticsTree,aestheticsSourceGroups} from './aesthetics-tree.mjs';
import {aestheticsTimeline} from './space-timeline.mjs';
import {scaleLabelClear} from '../../constructor/scale-labels.mjs';
import {readChronology} from '../../../../bonsai/lib/chronology.mjs';
import {jaccardDistance} from './space-proximity.mjs';
import {colonnade} from './space-gatherings.mjs';
import {makeAesthetics} from './aesthetics-library.mjs';
describe('Aesthetics field projection',()=>{
  const result=makeAesthetics({field,library:catalog,chronology,fragments:places}),byId=new Map(result.nodes.map(node=>[node.id,node]));
  it('shows each conception and every cited place on each of its area routes',()=>{
    for(const item of field.conceptions){const node=byId.get(`aesthetics:reading:${item.id}`);expect(node).toBeDefined();expect(node.sourceNodeIds).toHaveLength(item.formula.cite.length);for(const id of node.sourceNodeIds)expect(byId.get(id)?.kind).toBe('fragment');for(const area of item.areas)expect(result.atlas.routes.find(route=>route.id===area)?.steps.map(step=>step.nodeId)).toContain(node.id);}
  });
  it('names each source star by its own place label in the reader’s language',async()=>{
    const cite={w:'baruch-spinoza.ethics',loc:'III def3 p132'},node=byId.get(`aesthetics:source:${cite.w}:${cite.loc}`);
    for(const locale of ['ru','en']){const fragments=createFragments({profile:'aesthetics',locale:()=>locale,t:key=>key});expect(node.title[locale]).toBe(await fragments.placeLabel(cite));}
    for(const node of result.nodes.filter(node=>node.kind==='fragment')){const cite=node.body.cite[0],label=places.places?.[cite.w]?.[cite.loc]?.label;if(label)expect(node.title).toEqual(label);}
  });
  it('seeds the bounded saved graph',()=>{const model=createConstructorModel(result,{storage:null});try{model.seedAtlas();expect(model.getState().nodes.length).toBeGreaterThan(0);}finally{model.dispose();}});
  it('places distinct stars in their strata, with groups in the crown and works by life',()=>{
    const layout=aestheticsTree({library:result,field,catalog,chronology}),seen=new Set();
    for(const node of result.nodes){const p=layout.positions.get(node.id);expect(p?.every(Number.isFinite)).toBe(true);expect(seen.has(JSON.stringify(p)),node.id).toBe(false);seen.add(JSON.stringify(p));}
    for(const area of field.areas.filter(a=>byId.has(`aesthetics:route:${a.id}`))){const [x,y]=layout.positions.get(`aesthetics:route:${area.id}`),center=('ABCD'.indexOf(area.group)-1.5)*440;expect(y).toBeLessThan(-450);expect(Math.abs(x-center)).toBeLessThanOrEqual(190);}
    expect(layout.names.map(name=>name.id)).toEqual(['group-A','group-B','group-C','group-D']);
    for(const [i,name] of layout.names.entries()){const group='ABCD'[i];expect(name.title).toEqual({ru:`Группа ${group}`,en:`Group ${group}`,es:`Grupo ${group}`,de:`Gruppe ${group}`,'zh-Hans':`${group}组`});expect(name.center[0]).toBe((i-1.5)*440);}
    const heights=field.areas.filter(area=>byId.has(`aesthetics:route:${area.id}`)).map(area=>layout.positions.get(`aesthetics:route:${area.id}`)[1]);expect(Math.max(...heights)-Math.min(...heights)).toBeGreaterThan(100);
    const crown=field.areas.filter(area=>byId.has(`aesthetics:route:${area.id}`)).map(area=>layout.positions.get(`aesthetics:route:${area.id}`));
    const trunk=result.nodes.filter(node=>node.kind==='concept').map(node=>layout.positions.get(node.id));
    expect(Math.max(...crown.map(p=>p[1]))).toBeLessThan(Math.min(...trunk.map(p=>p[1])));
    expect(Math.max(...crown.map(p=>p[0]))-Math.min(...crown.map(p=>p[0]))).toBeGreaterThan(Math.max(...trunk.map(p=>p[0]))-Math.min(...trunk.map(p=>p[0])));
    for(const group of 'ABCD'){expect(field.areas.some(area=>area.group===group&&byId.has(`aesthetics:route:${area.id}`))).toBe(true);expect(layout.names.find(name=>name.id===`group-${group}`)?.title.ru).toBe(`Группа ${group}`);}
    for(const concept of result.nodes.filter(n=>n.kind==='concept')){const [x,y,z]=layout.positions.get(concept.id);expect(Math.hypot(x,z)).toBeLessThan(200);expect(y).toBeLessThan(0);}
    const works=Object.entries(catalog.works).sort(([,a],[,b])=>Number(chronology.situations[chronology.lives[a.agents[0]]].dating.interval.start)-Number(chronology.situations[chronology.lives[b.agents[0]]].dating.interval.start));
    for(const [i,[id]] of works.entries()){const [x,y,z]=layout.positions.get(`aesthetics:catalog:${id}`);expect(y).toBeGreaterThan(0);const angle=(Math.atan2(x,z)+2*Math.PI)%(2*Math.PI);expect(Math.min(Math.abs(angle-2*Math.PI*i/works.length),Math.abs(angle-2*Math.PI*i/works.length-2*Math.PI))).toBeLessThan(1e-8);}
  });
  it('lays each source and reading beside its work',()=>{
    const groups=aestheticsSourceGroups({library:result,chronology}),view=colonnade({authors:groups});
    for(const item of field.conceptions){const work=groups.flatMap(a=>a.works).find(w=>w.id===`aesthetics:catalog:${item.work}`);expect(work.front).toContain(`aesthetics:reading:${item.id}`);for(const id of byId.get(`aesthetics:reading:${item.id}`).sourceNodeIds)expect(work.up).toContain(id);}
    for(const group of groups)for(const work of group.works)for(const id of [work.id,...work.up,...work.front])expect(view.positions.has(id)).toBe(true);
  });
  it('places each reading on its dated work and in the first area group, within the displayed scale',()=>{
    const time=aestheticsTimeline({field,library:result,chronology}),clock=readChronology(chronology);
    expect(time.lanes.map(lane=>lane.id)).toEqual(['A','B','C','D']);
    for(const locale of ['ru','en','es','de','zh-Hans'])for(const lane of time.lanes){
      const label=time.labels(locale).find(item=>item.id===`time:lane:${lane.id}`);
      expect(label.title).toBe(({ru:`Группа ${lane.id}`,en:`Group ${lane.id}`,es:`Grupo ${lane.id}`,de:`Gruppe ${lane.id}`,'zh-Hans':`${lane.id}组`})[locale]);
      expect(label.ref).toBeUndefined();expect(label.hint).toBeUndefined();
    }
    for(const item of field.conceptions){
      const id=`aesthetics:reading:${item.id}`,span=clock.span(chronology.compositions.works[item.work]);
      const [x,y]=time.positions.get(id),group=field.areas.find(area=>area.id===item.areas[0]).group,lane=time.lanes.find(row=>row.id===group);
      expect(time.timeOf(id).composition).toBe(chronology.compositions.works[item.work]);
      expect(Math.abs(y-lane.y),id).toBeLessThan(35);
      if(span){expect(x,id).toBeGreaterThanOrEqual(time.xOfYear(span.from)-1e-9);expect(x,id).toBeLessThanOrEqual(time.xOfYear(span.to)+1e-9);expect(span.from).toBeGreaterThanOrEqual(time.domain.min);expect(span.to).toBeLessThanOrEqual(time.domain.max);}
      else expect(time.isOutside(id)).toBe(true);
    }
    const [a,b]=field.conceptions;expect(jaccardDistance(a.of,b.of)).toBeGreaterThanOrEqual(0);expect(jaccardDistance(a.of,a.of)).toBe(0);
  });
  it('gives active centuries room and keeps axis labels apart in a narrow view',()=>{
    const time=aestheticsTimeline({field,library:result,chronology});
    const empty=time.xOfYear(101)-time.xOfYear(1),dense=time.xOfYear(1901)-time.xOfYear(1801);
    expect(dense).toBeGreaterThan(empty);
    for(const locale of ['ru','en','es','de','zh-Hans']){
      const labels=time.labels(locale).filter(label=>label.variant==='axis'),visible=[];
      // A 360px map after the row-name strip; use conservative widths for the axis font.
      for(const label of labels){expect(label.noOverlap).toBe(true);const x=(label.center[0]-time.bounds[0])/(time.bounds[1]-time.bounds[0])*360,width=Math.max(24,Array.from(label.title).length*9+12),area={left:x-width/2,right:x+width/2,top:0,bottom:16};if(area.left>=0&&area.right<=360&&scaleLabelClear(area,visible))visible.push(area);}
      expect(visible.length).toBeGreaterThan(1);
      for(let i=1;i<visible.length;i++)expect(visible[i].left).toBeGreaterThanOrEqual(visible[i-1].right+2);
    }
  });
  it('keeps an undated reading beyond the year scale in its own row',()=>{
    const item=field.conceptions[0],id=`aesthetics:reading:${item.id}`,changed=structuredClone(chronology);
    changed.situations[changed.compositions.works[item.work]].dating={kind:'unknown-date',certainty:'unknown'};
    const time=aestheticsTimeline({field,library:result,chronology:changed}),[x,y]=time.positions.get(id);
    const group=field.areas.find(area=>area.id===item.areas[0]).group,lane=time.lanes.find(row=>row.id===group);
    expect(time.isOutside(id)).toBe(true);expect(x).toBeGreaterThan(time.xOfYear(time.domain.max));expect(Math.abs(y-lane.y)).toBeLessThan(35);
    expect(time.labels('ru').find(label=>label.id==='time:undated')?.title).toBe('дата неизвестна');
  });
  it('holds only grounded areas, concepts and relations',()=>{
    const used=new Set(field.conceptions.flatMap(item=>item.of));
    for(const item of field.concepts)expect(byId.has(`aesthetics:catalog:${item.id}`)).toBe(used.has(item.id));
    for(const area of field.areas)expect(byId.has(`aesthetics:route:${area.id}`)).toBe(field.conceptions.some(item=>item.areas.includes(area.id)));
    for(const link of field.links){const shown=result.atlas.edges.some(edge=>edge.id===`aesthetics:relation:${link.id}`);expect(shown).toBe(used.has(link.from)&&used.has(link.to)&&!!link.grounds?.length&&link.grounds.every(id=>field.conceptions.some(item=>item.id===id)));}
  });
});
