import {describe,expect,it} from 'vitest';
import collection from '../../../../bonsai/antiquity/collection.json' with {type:'json'};
import chronology from '../../../../bonsai/antiquity/chronology.json' with {type:'json'};
import {antiquityTimeline} from './space-timeline.mjs';
import {datingKind,measureAntiquity,returnsIn} from './space-measure.mjs';

describe('measure and completeness',()=>{
  const entries=[...collection.entities,...collection.eras,...collection.passages];
  const routes=collection.branches.map(branch=>({id:branch.id,title:branch.rootTitle,steps:branch.nodes.map(node=>({nodeId:`antiquity:reading:${node.id}`}))}));
  const timeline=antiquityTimeline({routes,entries,chronology});
  it('counts every move once, by the kind of its dating',()=>{
    const m=measureAntiquity({routes,entries,timeline});
    const total=Object.values(m.dating.kinds).reduce((sum,n)=>sum+n,0);
    expect(total).toBe(m.total);expect(m.total).toBe(timeline.positions.size);
    expect(Object.values(m.dating.certainty).reduce((sum,n)=>sum+n,0)).toBe(m.total);
  });
  it('counts a dating within a phase or a life as firm as that span, never firmer',()=>{
    const m=measureAntiquity({routes,entries,timeline}),reader=timeline.reader;
    let exact=0,firmerThanOwn=0;
    for(const id of timeline.positions.keys()){
      const situation=timeline.timeOf(id).composition,own=reader.situation(situation).dating.certainty,span=timeline.spanOf(id);
      if(span.certainty==='exact')exact++;
      if(own==='exact'&&span.certainty!=='exact')firmerThanOwn++;
    }
    expect(m.dating.certainty.exact??0).toBe(exact);
    // Works certain to fall within an approximate life exist, and are not counted exact.
    expect(firmerThanOwn).toBeGreaterThan(0);
    const menoeceus=[...timeline.positions.keys()].find(id=>timeline.timeOf(id).workId==='menoeceus');
    expect(datingKind(timeline.timeOf(menoeceus).composition,reader)).toBe('relative-life');
    expect(timeline.spanOf(menoeceus).certainty).toBe(reader.span(reader.lifeOf('epicurus')).certainty);
  });
  it('counts a branch\'s steps back in time',()=>{
    expect(returnsIn([1,2,3])).toBe(0);expect(returnsIn([3,1,2,0])).toBe(2);expect(returnsIn([1,NaN,0])).toBe(1);
  });
});
