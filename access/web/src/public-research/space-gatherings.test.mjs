import {describe,expect,it} from 'vitest';
import {bookSpiral,colonnade,imageDome,meaningTiers,tensionScales} from './space-gatherings.mjs';

const len=p=>Math.hypot(...p);
describe('the lenses that gather, in volume',()=>{
  it('climbs the book: a later chapter stands higher, a later part a turn above, a reading farther out than its text',()=>{
    const at={a:{part:1,chapter:2},b:{part:1,chapter:20},c:{part:2,chapter:25},t:{part:1,chapter:2},d:null};
    const v=bookSpiral({stars:Object.keys(at),chapterOf:id=>at[id],rungOf:id=>id==='t'?0:1});
    expect(v.positions.get('b')[1]).toBeLessThan(v.positions.get('a')[1]);
    expect(v.positions.get('c')[1]).toBeLessThan(v.positions.get('b')[1]);
    const flat=p=>Math.hypot(p[0],p[2]);expect(flat(v.positions.get('a'))).toBeGreaterThan(flat(v.positions.get('t')));
    expect(v.positions.has('d')).toBe(true);
  });
  it('raises a work\'s passages up its column and sets its readings before it, its author at its foot',()=>{
    const v=colonnade({authors:[{id:'plato',works:[{id:'rep',up:['p1','p2'],front:['m1']}]},{id:null,works:[{id:'anon',up:[],front:[]}]}]});
    const [wx,wy,wz]=v.positions.get('rep'),[px,py,pz]=v.positions.get('p2'),[,p1y]=v.positions.get('p1');
    expect([px,pz]).toEqual([wx,wz]);expect(py).toBeLessThan(p1y);expect(p1y).toBeLessThan(wy);
    expect(Math.hypot(v.positions.get('m1')[0]-wx,v.positions.get('m1')[2]-wz)).toBeCloseTo(30);
    expect(v.positions.get('plato')[1]).toBeGreaterThan(wy);expect(v.positions.has('anon')).toBe(true);
  });
  it('raises each kind to its tier, keeps a circle in its sector, and lays out the same way twice',()=>{
    const tier={t:0,r1:1,r2:1,r3:1,c:2,q:3},group={t:'a',r1:'a',r2:'a',r3:'b',c:'a',q:'b'},args={stars:Object.keys(tier),tierOf:id=>tier[id],groupOf:id=>group[id],links:[{from:'r1',to:'c'},{from:'r2',to:'c'}]};
    const v=meaningTiers(args),y=id=>v.positions.get(id)[1],flat=id=>Math.hypot(v.positions.get(id)[0],v.positions.get(id)[2]);
    expect(y('t')).toBeGreaterThan(y('r1'));expect(y('r1')).toBeGreaterThan(y('c'));expect(y('c')).toBeGreaterThan(y('q'));
    expect(flat('t')).toBeLessThan(flat('r1'));expect(flat('c')).toBeLessThan(flat('q'));
    const at=id=>Math.atan2(v.positions.get(id)[2],v.positions.get(id)[0]),apart=(a,b)=>Math.abs(Math.atan2(Math.sin(at(a)-at(b)),Math.cos(at(a)-at(b))));
    expect(apart('r1','c')).toBeLessThan(apart('r3','c'));
    expect(meaningTiers(args).positions.get('c')).toEqual(v.positions.get('c'));
  });
  it('hangs grounds and objections on the two arms of a disputed star, and threads a star that hangs twice',()=>{
    const v=tensionScales({pivots:[{id:'p',grounds:['g'],against:['x','y']},{id:'q',grounds:[],against:['x']}]});
    const [px,py,pz]=v.positions.get('p'),g=v.positions.get('g'),x=v.positions.get('x');
    expect(g[1]).toBeGreaterThan(py);expect(x[1]).toBeGreaterThan(py);
    const side=p=>(p[0]-px)*-Math.sin(Math.atan2(pz,px))+(p[2]-pz)*Math.cos(Math.atan2(pz,px));
    expect(Math.sign(side(g))).toBe(-Math.sign(side(x)));
    expect(v.positions.has('q')).toBe(true);expect(v.positions.get('x')).toEqual(x);
  });
  it('lifts an image that reaches concepts higher, and sets the concept over it',()=>{
    const v=imageDome({images:[{id:'low',chapter:3,reach:0},{id:'high',chapter:10,reach:1}],concepts:[{id:'c',images:['high']}]});
    expect(v.positions.get('high')[1]).toBeLessThan(v.positions.get('low')[1]);
    expect(v.positions.get('c')[1]).toBeLessThan(v.positions.get('high')[1]);
  });
});
