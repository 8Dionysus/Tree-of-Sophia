import {describe,expect,it} from 'vitest';
import {besidePosition,disputes,ownTitle,readingsAround,restsOn,routeStars,threadsIntoDark} from './space-play.mjs';

const rungs={q:'reading',r1:'reading',r2:'reading',c1:'comparison',s1:'source',s2:'source',p:'personal'};
const rungOf=id=>rungs[id]??null;
const edges=[
  {from:'r2',to:'q',kind:'compares'},{from:'r1',to:'q',kind:'interprets'},{from:'q',to:'c1',kind:'develops'},
  {from:'q',to:'s1',kind:'supports'},{from:'p',to:'q',kind:'relates'},{from:'r1',to:'q',kind:'echoes'},
  {from:'r1',to:'s1',kind:'interprets'},{from:'s2',to:'r1',kind:'supports'},{from:'r1',to:'r2',kind:'contrasts'},{from:'c1',to:'r1',kind:'questions'},
];

describe('your own reading first',()=>{
  it('finds the readings around a question, nearest kinds first, each once, never a source or your own record',()=>{
    expect(readingsAround('q',{edges,rungOf})).toEqual([{id:'r1',kind:'interprets'},{id:'c1',kind:'develops'},{id:'r2',kind:'compares'}]);
  });
  it('says what a reading rests on and what disputes it',()=>{
    expect(restsOn('r1',{edges,rungOf})).toEqual(['s1','s2']);
    expect(disputes('r1',{edges})).toEqual(['r2','c1']);
    expect(disputes('s1',{edges})).toEqual([]);
  });
  it('titles a long line at a word, and keeps a short one whole',()=>{
    expect(ownTitle('  Воля  хочет   своей судьбы ')).toBe('Воля хочет своей судьбы');
    const long='Мне кажется, что здесь важно не столько само возвращение, сколько то, кто его узнаёт и каким голосом об этом говорит';
    const title=ownTitle(long);
    expect(title.endsWith('…')).toBe(true);expect(title.length).toBeLessThanOrEqual(91);expect(long.startsWith(title.slice(0,-1))).toBe(true);
  });
  it('stands a reader’s star beside another one, never on it',()=>{
    for(let index=0;index<4;index++){const [x,y,z]=besidePosition([100,50,12],index);expect(Math.hypot(x-100,y-50)).toBeGreaterThan(20);expect(z).toBe(12);}
  });
});

describe('the edges of what is known',()=>{
  const positions={a:[0,0,0],b:[100,0,0],c:[0,-50,0],d:[0,0,0],e:null};
  const links=[{from:'a',to:'b',kind:'develops'},{from:'c',to:'a',kind:'interprets'},{from:'a',to:'d',kind:'echoes'},{from:'a',to:'e',kind:'supports'},{from:'a',to:'f',kind:'relates'}];
  it('leads from a chosen star toward the stars the space does not show, in their direction',()=>{
    const threads=threadsIntoDark('a',{edges:links,shown:new Set(['a','b']),positionOf:id=>positions[id]??null});
    expect(threads.map(thread=>thread.id)).toEqual(['c','d']);
    expect(threads[0].direction).toEqual([0,-1]);
    expect(Math.hypot(...threads[1].direction)).toBeCloseTo(1);
  });
  it('gives no thread from a star without a place',()=>{
    expect(threadsIntoDark('e',{edges:links,positionOf:id=>positions[id]??null})).toEqual([]);
  });
});

describe('a route lights its way',()=>{
  it('names the stars of its steps the space shows, in order, each once',()=>{
    const steps=[{nodeId:'x'},{nodeId:'y'},{graphNodeId:'z'},{nodeId:'x'},{}];
    expect(routeStars(steps,{graphIdOf:id=>`material:${id}`,shown:new Set(['material:x','material:z'])})).toEqual(['material:x','material:z']);
  });
});
