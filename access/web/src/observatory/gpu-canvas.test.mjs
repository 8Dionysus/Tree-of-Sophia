import {expect,test,vi} from 'vitest';

// The painter's records are what reaches the GPU; a context that accepts every call is enough to read them.
const gl=new Proxy({},{get:(_,key)=>key==='getShaderParameter'||key==='getProgramParameter'?()=>true:key==='isContextLost'?()=>false:/^[A-Z0-9_]+$/.test(String(key))?0:()=>({})});
vi.stubGlobal('document',{createElement:()=>({width:0,height:0,getContext:()=>({measureText:()=>({width:0}),drawImage(){},clearRect(){}})})});
vi.stubGlobal('addEventListener',()=>{});
const {SophiaGpuCanvas}=await import('./gpu-canvas.js');

const painter=()=>{const p=new SophiaGpuCanvas({width:800,height:600,getContext:()=>gl,addEventListener(){},dataset:{}});p.beginFrame();return p;};
const glow={width:32,height:32};
const star=(x,y,more={})=>({x,y,color:'#a8cde5',alpha:.9,radius:2.4,core:.77,image:glow,glowX:-23,glowY:-23,glowSize:46,glowAlpha:.9,ray:12,from:.25,rayLight:.52,lineScale:1,...more});
const disc=(p,x,y)=>{p.beginPath();p.arc(x,y,3,0,Math.PI*2);p.fill();};

test('stars between other strokes keep their place in the order of drawing',()=>{
  const p=painter();p.setTransform(1.25,0,0,1.25,0,0);
  disc(p,10,10);p.star(star(100,80));p.star(star(140,90));disc(p,20,20);p.star(star(160,95));
  expect(p.groups.map(g=>[!!g.stars,g.start,g.count])).toEqual([[false,0,4],[true,0,2],[false,4,4],[true,2,1]]);
  const record=p.starData.subarray(0,24);
  expect(Array.from(record.subarray(0,4))).toEqual([125,100,1.25,Math.fround(2.4)]);
  expect(Array.from(record.subarray(4,8))).toEqual([-23,-23,46,Math.fround(.9)]);
  expect(record[19]).toBe(Math.fround(.52));
});

test('a kept stretch of the scene lays its stars again with its strokes',()=>{
  const p=painter();
  disc(p,1,1);const from=p.mark();p.star(star(50,50));disc(p,2,2);p.star(star(60,60,{rayLight:0}));const to=p.mark();disc(p,3,3);
  const kept=p.keep(from,to);
  const quads=Array.from(p.data.subarray(4*11,8*11)),stars=Array.from(p.starData.subarray(0,2*24));
  p.beginFrame();disc(p,9,9);
  expect(p.replay(kept)).toBe(true);
  expect(p.groups.map(g=>[!!g.stars,g.start,g.count])).toEqual([[false,0,4],[true,0,1],[false,4,4],[true,1,1]]);
  expect(Array.from(p.data.subarray(4*11,8*11))).toEqual(quads);
  expect(Array.from(p.starData.subarray(0,2*24))).toEqual(stars);
});

test("the paper's uniforms travel once, until the paper or the canvas changes",()=>{
  const calls=[];
  // As in WebGL, a uniform is set at the location the program gave for it, and nothing else stands in for one.
  const located=new WeakSet(),location=name=>{const value={name};located.add(value);return value;};
  const counting=new Proxy({},{get:(_,key)=>key==='getShaderParameter'||key==='getProgramParameter'?()=>true:key==='isContextLost'?()=>false:key==='getUniformLocation'?(_program,name)=>location(name):/^[A-Z0-9_]+$/.test(String(key))?0:
    /^uniform/.test(String(key))?(at,...rest)=>{if(!located.has(at))throw new TypeError(`${String(key)} needs a uniform location`);calls.push(String(key));return rest;}:()=>{calls.push(String(key));return {};}});
  const canvas={width:800,height:600,getContext:()=>counting,addEventListener(){},dataset:{}};
  const p=new SophiaGpuCanvas(canvas);
  const paper={steps:[{matrix:[1,0,0,0,1,0,0,0,1],offset:[0,0,0]}],stops:[[1,1,1,0]],center:[400,300],radii:[500,400]};
  expect(p.setPaper(paper)).toBe(true);
  const frame=()=>{calls.length=0;p.beginFrame();disc(p,10,10);p.endFrame();return [calls.filter(name=>name==='uniformMatrix3fv').length,calls.filter(name=>name==='drawArrays').length];};
  expect(frame()).toEqual([1,1]);
  expect(frame()).toEqual([0,1]);
  canvas.height=640;expect(frame()).toEqual([1,1]);
  p.setPaper({...paper});expect(frame()).toEqual([1,1]);
});
