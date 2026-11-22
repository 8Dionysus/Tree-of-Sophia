import {describe,expect,it} from 'vitest';
import {lensMembers} from './lens-preview.mjs';

const graph={nodes:[
  {id:'a',materialId:'m:a'},{id:'b',materialId:'m:b'},{id:'c',materialId:'m:c'},
  {id:'personal:1',ownerType:'personal-hypothesis'},
]};
const positions=(...ids)=>({positions:new Map(ids.map(id=>[id,{x:0,y:0}]))});

describe('the stars a lens would show',()=>{
  it('lights nothing apart for the tree, which shows them all',()=>{
    expect(lensMembers('tree',{graph})).toBeNull();
  });

  it('reads the scale of time and the map by the stars they place',()=>{
    expect([...lensMembers('time',{graph,timeline:positions('m:a','m:c')})]).toEqual(['a','c']);
    expect([...lensMembers('space',{graph,geography:positions('m:b')})]).toEqual(['b']);
  });

  it('reads proximity by the stars it measures',()=>{
    expect([...lensMembers('proximity',{graph,starIds:['m:b','m:c']})]).toEqual(['b','c']);
  });

  it('keeps the reader\'s own records in a lens that gathers',()=>{
    expect([...lensMembers('sources',{graph,gathering:positions('m:a')})]).toEqual(['a','personal:1']);
  });

  it('asks any other lens for its view, and only then',()=>{
    let asked=0;
    const view=()=>{asked++;return {nodes:[{id:'b'},{id:'gone'}]};};
    expect([...lensMembers('works',{graph,view})]).toEqual(['b']);
    expect(asked).toBe(1);
    lensMembers('time',{graph,timeline:positions(),view});
    expect(asked).toBe(1);
  });
});
