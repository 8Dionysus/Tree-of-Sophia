import {describe,expect,it} from 'vitest';
import {ASPECT,aspectFocus,aspectLayout,familyOf} from './space-aspect.mjs';

const node=(id,x=0,y=0,z=0)=>({id,position:[x,y,z]});
const link=(id,from,to,extra={})=>({id,from,to,kind:'relates',label:id,...extra});
const view=(nodes,edges)=>({nodes,edges,clusters:[{id:'group'}],planes:[{id:'plane'}]});

describe('aspect of a chosen star',()=>{
  const space=view([node('a',10,20,-220),node('b'),node('c'),node('d'),node('far',500,500,0)],[link('ba','b','a'),link('ac','a','c'),link('ad','a','d'),link('cd','c','d')]);
  const turned=aspectLayout(space,{nodeId:'a'});
  const at=id=>turned.nodes.find(item=>item.id===id);
  it('stands what leads in on the left and what leads on on the right, in front of the space',()=>{
    expect(at('b').position[0]).toBeLessThan(at('a').position[0]);
    expect(at('c').position[0]).toBeGreaterThan(at('a').position[0]);
    expect(at('c').position[0]).toBe(at('d').position[0]);
    expect(at('a').position[2]).toBe(ASPECT.forward);
    expect(new Set(['b','c','d'].map(id=>at(id).position[1])).size).toBe(3);
  });
  it('names each of its links beside the star it reaches and mutes the others',()=>{
    const edge=id=>turned.edges.find(item=>item.id===id);
    expect(edge('ba').labelAt).toMatchObject({id:'b',align:'start'});
    expect(edge('ac').labelAt).toMatchObject({id:'c',align:'end'});
    expect(edge('cd').muted).toBe(true);
  });
  it('steps the rest back and leaves the space as it was',()=>{
    expect(at('far')).toMatchObject({dim:true,position:[500,500,ASPECT.back]});
    expect(turned.planes).toEqual([]);
    expect(turned.aspect).toMatchObject({kind:'node',focus:['a']});
    expect(space.nodes.find(item=>item.id==='b').position).toEqual([0,0,0]);
  });
  it('leaves a view without a choice unchanged',()=>{
    expect(aspectLayout(space,{})).toBe(space);
    expect(aspectFocus(space,{nodeId:'missing'})).toBeNull();
  });
});

describe('a crowded side',()=>{
  const others=Array.from({length:14},(_,index)=>`n${index}`);
  const family=index=>index<9?{family:'ground',family_label:{ru:'основание',en:'ground'}}:{family:`f${index}`,family_label:{ru:`семья ${index}`}};
  const space=view([node('a'),...others.map(id=>node(id))],others.map((id,index)=>link(`l${index}`,id,'a',{coreRecord:{relation_family:family(index)}})));
  it('closes its largest family into a header with its stars gathered and unnamed',()=>{
    const turned=aspectLayout(space,{nodeId:'a'});
    const header=turned.clusters.find(item=>item.ref==='aspect:in:family:ground');
    expect(header).toMatchObject({variant:'facet',title:'основание · 9',tone:''});
    expect(turned.nodes.find(item=>item.id==='n0').quiet).toBe(true);
    expect(turned.edges.find(item=>item.id==='l0').muted).toBe(true);
    expect(turned.edges.find(item=>item.id==='l12').labelAt).toBeTruthy();
  });
  it('opens the family the reader chose in rows of its own',()=>{
    const turned=aspectLayout(space,{nodeId:'a',open:{in:'family:ground'}});
    expect(turned.clusters.find(item=>item.ref==='aspect:in:family:ground').tone).toBe('open');
    expect(turned.edges.find(item=>item.id==='l0').labelAt).toMatchObject({id:'n0'});
    expect(turned.nodes.find(item=>item.id==='n0').quiet).toBe(false);
  });
  it('names a family by the registry, else by the kind of its link',()=>{
    expect(familyOf({coreRecord:{relation_family:{family:'x',family_label:'X'}}})).toEqual({key:'family:x',label:'X'});
    expect(familyOf({kind:'supports',tag:'Поддерживает'})).toEqual({key:'kind:supports',label:'Поддерживает'});
  });
});

describe('aspect of a chosen link',()=>{
  const space=view([node('a',-50),node('b',50),node('c'),node('s')],[link('ab','a','b'),link('ac','a','c'),link('as','a','s'),link('sb','s','b')]);
  it('puts its stars side by side with its verb between them and what both share below',()=>{
    const turned=aspectLayout(space,{edgeId:'ab'}),at=id=>turned.nodes.find(item=>item.id===id).position;
    expect(at('a')[1]).toBe(at('b')[1]);
    expect(at('a')[0]).toBeLessThan(at('b')[0]);
    expect(at('s')[1]).toBeGreaterThan(at('a')[1]);
    expect(at('c')[0]).toBeLessThan(at('a')[0]);
    expect(turned.edges.find(item=>item.id==='ab').labelAt).toEqual({mid:true});
  });
  it('stands upright in a narrow area, its second star under its first',()=>{
    const turned=aspectLayout(space,{edgeId:'ab',narrow:true}),at=id=>turned.nodes.find(item=>item.id===id).position;
    expect(at('a')[0]).toBe(at('b')[0]);
    expect(at('b')[1]).toBeGreaterThan(at('a')[1]);
    expect(turned.edges.find(item=>item.id==='ab').labelAt).toEqual({mid:true,align:'start'});
  });
});
