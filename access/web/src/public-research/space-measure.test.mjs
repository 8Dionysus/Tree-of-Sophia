import {beforeAll,describe,expect,it} from 'vitest';
import {readFileSync} from 'node:fs';
import * as rules from '../../../deploy/cloudflare-worker/generated/tos_web_rules.js';
import {configureResearchOperations} from './research-operations.mjs';
beforeAll(()=>{rules.initSync({module:new WebAssembly.Module(readFileSync(new URL('../../../deploy/cloudflare-worker/generated/tos_web_rules_bg.wasm',import.meta.url)))});configureResearchOperations(rules);});
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

it('research operations preserve direction, selected scope and unknown measurements', async()=>{
  const {runResearchOperation}=await import('./research-operations.mjs');
  const request=(operation,input)=>({operation,scope:'selected',versions:{projection:'v1'},input});
  const edges=[{from:'a',to:'b',type:'supports',ref:'source:1'},{from:'b',to:'c',type:'supports',ref:'source:2'}];
  const path=runResearchOperation(request('directed_path',{from:'a',to:'c',allowed_types:['supports'],edges}));
  expect(path.result.transitions).toBe(2);expect(path.result.edges[0].ref).toBe('source:1');
  expect(runResearchOperation(request('directed_path',{from:'c',to:'a',allowed_types:['supports'],edges})).result.found).toBe(false);
  expect(runResearchOperation(request('common_grounds',{left:[],right:['r']})).result.jaccard_distance).toBeNull();
  const time=runResearchOperation(request('time_place',{left:{span:null,point:[120,30]},right:{span:null,point:[121,30]}}));
  expect(time.result.time_midpoint_distance_years).toBeNull();expect(time.result.place_distance_km).toBeGreaterThan(0);
});

it('native research operations preserve evidence, direction and unknowns',async()=>{
  const {runResearchOperation}=await import('./research-operations.mjs');
  const run=(operation,input)=>runResearchOperation({operation,scope:'selected',versions:{projection:'v1'},input}).result;
  expect(run('distribution',{witness:'de',search_kind:'word',items:[{matches:true,group:'a',location:'I.1'},{matches:true,group:'a'},{matches:false}]})).toEqual({witness:'de',search_kind:'word',denominator:3,count:2,groups:{a:2},locations:['I.1',null],unlocated:1});
  const common=run('common_grounds',{left:['a','b','a'],right:['b','c']});expect(common).toMatchObject({common:['b'],only_left:['a'],only_right:['c']});expect(common.jaccard_distance).toBeCloseTo(2/3);
  expect(run('interpretation_comparison',{axes:[{id:'x'},{id:'y'}],left:{x:1},right:{x:2}}).features.map(x=>x.status)).toEqual(['different','unknown']);
  expect(run('argument_structure',{steps:[{id:'s'}],objections:[{target_step:'s'},{target_step:'missing'}]}).unattached_objections).toEqual([{target_step:'missing'}]);
  expect(run('dependency_impact',{changed:'a',dependencies:[{from:'a',to:'b'},{from:'b',to:'a'}]}).affected).toEqual(['b']);
  expect(run('time_place',{left:{span:{from:1,to:3},point:[0,0]},right:{span:{from:5,to:7},point:[0,1]}}).time_midpoint_distance_years).toBe(4);
});
