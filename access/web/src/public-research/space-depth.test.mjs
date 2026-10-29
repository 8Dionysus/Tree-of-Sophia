import {describe,expect,it} from 'vitest';
import {depthLayout} from './space-depth.mjs';
import {bonsaiFor,depthOf} from './bonsai.mjs';
import {ladderRung} from './space-legibility.mjs';

describe('depth on the lenses that group',()=>{
  const depth=depthOf('zarathustra');
  const nodes=[
    {id:'a',position:[0,0,12],step:'source'},{id:'b',position:[100,50,-7],step:'reading'},
    {id:'c',position:[-80,120,3],step:'reading'},{id:'d',position:[40,-60,0],step:'personal'},{id:'e',position:[5,5,5],step:'unknown'},
  ];
  it('lays each star at the depth of its step and keeps its place on the map',()=>{
    const {nodes:laid}=depthLayout(nodes,{stepOf:node=>node.step,depth});
    const z=Object.fromEntries(depth.steps.map(step=>[step.id,step.z]));
    expect(laid.map(node=>node.position)).toEqual([[0,0,z.source],[100,50,z.reading],[-80,120,z.reading],[40,-60,z.personal],[5,5,0]]);
    // The graph's own positions are not touched.
    expect(nodes[0].position).toEqual([0,0,12]);
  });
  it('puts each step that holds stars on a plane named by the step, deep to near',()=>{
    const {planes}=depthLayout(nodes,{stepOf:node=>node.step,depth,locale:'en'});
    expect(planes.map(plane=>plane.label)).toEqual(['Source','Reading','Personal']);
    expect(planes.map(plane=>plane.z)).toEqual([...planes.map(plane=>plane.z)].sort((a,b)=>a-b));
    const reading=planes.find(plane=>plane.id==='step:reading');
    expect(reading.x[0]).toBeLessThan(-80);expect(reading.x[1]).toBeGreaterThan(100);expect(reading.y[0]).toBeLessThan(50);expect(reading.y[1]).toBeGreaterThan(120);
    expect(depthLayout(nodes,{stepOf:node=>node.step,depth,locale:'ru'}).planes[0].label).toBe('Источник');
  });
  it('needs no planes for one step, and changes nothing without a declared depth',()=>{
    expect(depthLayout(nodes.slice(1,3),{stepOf:node=>node.step,depth}).planes).toEqual([]);
    expect(depthLayout(nodes,{stepOf:node=>node.step,depth:null})).toEqual({nodes,planes:[]});
  });
  it('reads the steps the stand already names: source, reading, comparison, personal',()=>{
    expect(ladderRung({material:{kind:'work'}})).toBe('source');
    expect(ladderRung({material:{kind:'concept',demo:true}})).toBe('reading');
    expect(ladderRung({material:{kind:'question',route:{id:'r'}}})).toBe('comparison');
    expect(ladderRung({node:{ownerType:'personal-hypothesis'}})).toBe('personal');
  });
});

describe('the depth each bonsai declares',()=>{
  for(const id of ['antiquity','zarathustra'])it(`${id}: orders the steps of interpretation deep to near, names them and rests turned`,()=>{
    const depth=depthOf(id),manifest=bonsaiFor(id).manifest;
    expect(depth).toBe(manifest.depth);
    expect(depth.positions).toBe('group');expect(depth.groups.ru).toBeTruthy();expect(depth.groups.en).toBeTruthy();
    expect(depth.steps.map(step=>step.id)).toEqual(['source','reading','comparison','personal']);
    for(let i=1;i<depth.steps.length;i++)expect(depth.steps[i].z,depth.steps[i].id).toBeGreaterThan(depth.steps[i-1].z);
    for(const step of depth.steps){expect(step.label.ru,step.id).toBeTruthy();expect(step.label.en,step.id).toBeTruthy();}
    // The rest pose is turned, so depth shows, and within the turns a drag can reach.
    expect(Math.abs(depth.rest.yaw)).toBeGreaterThan(.1);expect(Math.abs(depth.rest.yaw)).toBeLessThanOrEqual(.9);expect(Math.abs(depth.rest.pitch)).toBeLessThanOrEqual(.6);
  });
});
