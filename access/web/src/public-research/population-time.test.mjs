import {describe,expect,it} from 'vitest';
import {memberSpan,populationTimeline} from './population-time.mjs';

const dated=year=>({dating:{kind:'date-assertion',role:'historical-time',calendar:null,year_numbering:'astronomical',certainty:'exact',value:String(year)}});
// A population whose members are known by one phase each, dated to one year.
function points(...years){
  const members={},situations={};
  years.forEach((year,i)=>{members[`m${i}`]={founded:'B1:1',grew_from:['B1:1'],phases:[`p${i}`]};situations[`p${i}`]=dated(year);});
  return {population:{members,batches:{B1:{node:{region:'europe'}}}},chronology:{situations,lives:{}}};
}

describe('the Population on the scale of time',()=>{
  it('reads a member known by one year as that whole year',()=>{
    const {chronology}=points(1801);
    expect(memberSpan({id:'m0',phases:['p0']},{chronology,reader:{span:()=>null}})).toMatchObject({from:1801,to:1802});
  });

  it('gives a member at the first year of a century that century, not an empty scale',()=>{
    for(const years of [[1801],[1800,1801],[1802]]){
      const timeline=populationTimeline(points(...years));
      const centuries=timeline.labels('en').filter(label=>label.variant==='axis');
      expect(centuries.length).toBeGreaterThan(0);
      for(let i=0;i<years.length;i++){
        const x=timeline.xOf(`population:member:m${i}`);
        expect(Number.isFinite(x)).toBe(true);
        expect(x).toBeLessThan(timeline.xOfYear(timeline.domain.max));
      }
      expect(timeline.domain.max).toBeGreaterThan(Math.max(...years));
    }
  });
});
