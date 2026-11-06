import {describe,expect,it} from 'vitest';
import {proximityVolume,spaceVolume,timeVolume} from './space-volumes.mjs';

const timeline={positions:new Map([['a',[-100,-40,0]],['b',[50,-40,0]],['c',[50,-28,0]],['d',[200,40,0]]]),lanes:[{id:'one',title:'Одна',y:-40},{id:'two',title:'Две',y:40}],bounds:[-500,500],
  labels:()=>[{id:'time:rule:1',variant:'rule',center:[0,0,0],tone:'strong'},{id:'time:c1',variant:'axis',title:'I в.',center:[-50,0,0]},{id:'time:lane:one',variant:'lane',title:'Одна',center:[0,-40,0]}]};

describe('the lenses that measure, in volume',()=>{
  it('keeps the year across, sends the rows into depth and lifts a stacked star',()=>{
    const v=timeVolume(timeline);
    expect(v.positions.get('a')[0]).toBe(-100);
    expect(v.positions.get('a')[2]).toBeLessThan(v.positions.get('d')[2]);
    expect(v.positions.get('c')[1]).toBeLessThan(v.positions.get('b')[1]);
    expect(v.names.find(name=>name.id==='time:c1').variant).toBe('tick');
    expect(v.lines.some(line=>line.points.every(p=>p[1]===0))).toBe(true);
  });
  it('stands a star over its place at the height of its time',()=>{
    const geography={positions:new Map([['early',[10,20,0]],['late',[10,20,0]],['nowhere',[600,0,0]]]),map:{x:[-500,500],y:[-300,300]},placeOf:id=>id==='nowhere'?null:{place:'athens',point:[23.7,38]},toXY:()=>[10,20],labels:()=>[{id:'space:place:athens',variant:'place',title:'Афины',center:[10,20,0]}]};
    const v=spaceVolume(geography,{xOfTime:id=>({early:-400,late:300})[id]??null,timeBounds:[-500,500]});
    const [ex,ey,ez]=v.positions.get('early'),[,ly]=v.positions.get('late');
    expect([ex,ez]).toEqual([10,20]);expect(ly).toBeLessThan(ey);
    expect(v.names.find(name=>name.id==='space:place:athens').center).toEqual([10,0,20]);
  });
  it('lifts what came later and lowers what came earlier round the chosen star',()=>{
    const near={center:'c',radius:420,positions:new Map([['c',[0,0,0]],['before',[100,0,0]],['after',[0,100,0]]]),labels:()=>[{id:'near:ring:1',variant:'ring',radius:100,center:[0,0,0]}]};
    const v=proximityVolume(near,{xOfTime:id=>({c:0,before:-10,after:10})[id]});
    expect(v.positions.get('c')).toEqual([0,0,0]);
    expect(v.positions.get('after')[1]).toBeLessThan(0);expect(v.positions.get('before')[1]).toBeGreaterThan(0);
    expect(v.positions.get('after')[2]).toBe(100);
    expect(v.lines[0].points.every(p=>p[1]===0)).toBe(true);
  });
});
