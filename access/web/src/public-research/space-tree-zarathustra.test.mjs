import {describe,expect,it} from 'vitest';
import witnesses from '../../../../bonsai/zarathustra/witnesses.json' with {type:'json'};
import {TREE_LABEL_FROM} from './space-tree.mjs';
import {zarathustraTree} from './space-tree-zarathustra.mjs';

// A small library of the stand's shape: the book, two chapters' places, figures of the core, an image, concepts,
// a question asked of a concept, and two figure paths through the book.
const cite=c=>({w:'also-sprach-zarathustra',c,u:[]});
const node=(id,kind,chapters,extra={})=>({id,kind,coreCitations:chapters.map(cite),...extra});
const library={
  nodes:[
    {id:'work',kind:'work',inquiry:{grounds:[{cite:[cite('I.01')]}]}},
    node('chapter-p3.r2','chapter',['III.02']),node('moment','fragment',['III.02']),
    node('demo:dwarf','character',['III.02']),node('core:zarathustra','symbol',['I.01','IV.20']),node('core:jester','symbol',['I.06']),
    node('demo:gateway','symbol',['III.02']),node('demo:recurrence','concept',['III.13']),node('core:overman','concept',['I.03']),
    node('demo:circle-change','question',[],{inquiry:{grounds:[{cite:[cite('III.13')]}]}}),
    ...[0,1,2,3].map(k=>node(`core:jester-leap#${k}`,'fragment',['I.06'])),
    ...[0,1,2].map(k=>node(`core:dwarf-weight#${k}`,'fragment',['III.02'])),
  ],
  atlas:{edges:[{id:'q',from:'demo:recurrence',to:'demo:circle-change',kind:'questions'}]},
  space:{
    core:{figures:[{id:'zarathustra'},{id:'jester'},{id:'dwarf'}],materialIds:{zarathustra:'core:zarathustra',jester:'core:jester',dwarf:'demo:dwarf'}},
    routes:[
      {id:'core:jester-leap',coreChainId:'jester-leap',title:{ru:'Прыжок',en:'Leap'},steps:[0,1,2,3].map(k=>({nodeId:`core:jester-leap#${k}`})),
        transitions:[{fromNodeId:'core:jester-leap#0',toNodeId:'core:jester-leap#1',label:{ru:'объявляет',en:'claims'}}]},
      {id:'core:dwarf-weight',coreChainId:'dwarf-weight',title:{ru:'Тяжесть',en:'Weight'},steps:[0,1,2].map(k=>({nodeId:`core:dwarf-weight#${k}`}))},
      {id:'core:recurrence:chain:01',coreChainId:'recurrence:chain:01',steps:[{nodeId:'demo:gateway'},{nodeId:'demo:recurrence'}]},
    ],
  },
};
const tree=zarathustraTree({library,chapters:witnesses.chapters});
const at=id=>tree.positions.get(id);
const angle=([x,,z])=>Math.atan2(x,z);
const gap=(a,b)=>Math.abs(Math.atan2(Math.sin(a-b),Math.cos(a-b)));

describe('the whole Zarathustra bonsai as one tree',()=>{
  it('gives every star of the library a place of its own in three dimensions',()=>{
    for(const item of library.nodes)expect(at(item.id),item.id).toBeTruthy();
    expect(tree.positions.size).toBe(library.nodes.length);
    expect(new Set([...tree.positions.values()].map(p=>p.map(Math.round).join(':'))).size).toBe(tree.positions.size);
    for(const role of new Set(tree.roles.values()))expect(TREE_LABEL_FROM[role],role).toBeTypeOf('number');
  });
  it('stands the figures on the ground where they enter the book and hangs the book’s places under their chapter',()=>{
    for(const id of ['core:zarathustra','core:jester','demo:dwarf']){expect(tree.roles.get(id)).toBe('figure');expect(at(id)[1]).toBe(0);}
    const order=witnesses.chapters.map(c=>c.key),ground=id=>angle(at(id));
    // Going round from the prologue, Zarathustra enters before the jester and the jester before the dwarf.
    const from=ground('core:zarathustra'),turn=id=>((ground(id)-from)%(2*Math.PI)+2*Math.PI)%(2*Math.PI);
    expect(order.indexOf('I.06')).toBeLessThan(order.indexOf('III.02'));expect(turn('core:jester')).toBeLessThan(turn('demo:dwarf'));
    expect(at('moment')[1]).toBeGreaterThan(0);expect(gap(angle(at('moment')),ground('demo:dwarf'))).toBeLessThan(.08);
    expect(at('work')[1]).toBeGreaterThan(at('moment')[1]);expect(Math.hypot(at('work')[0],at('work')[2])).toBe(0);
  });
  it('raises images and concepts in the trunk, each toward the chapters it cites',()=>{
    expect(tree.regionOf('demo:gateway')).toBe('trunk');expect(tree.regionOf('demo:recurrence')).toBe('trunk');
    expect(at('demo:recurrence')[1]).toBeLessThan(at('demo:gateway')[1]);
    expect(gap(angle(at('demo:gateway')),angle(at('demo:dwarf')))).toBeLessThan(.1);
  });
  it('grows each figure’s path in the crown above the chapters it reads, further out with each move',()=>{
    const steps=[0,1,2,3].map(k=>`core:jester-leap#${k}`);
    for(const id of steps){expect(tree.roles.get(id)).toBe('step');expect(tree.regionOf(id)).toBe('crown');}
    const reach=id=>Math.hypot(at(id)[0],at(id)[2]);
    for(let k=1;k<steps.length;k++)expect(reach(steps[k])).toBeGreaterThan(reach(steps[k-1]));
    expect(gap(angle(at(steps[0])),angle(at('core:jester')))).toBeLessThan(Math.PI/2);
    expect(gap(angle(at('core:dwarf-weight#0')),angle(at('demo:dwarf')))).toBeLessThan(Math.PI/2);
    expect(at('demo:circle-change')[1]).toBeLessThan(Math.min(...steps.map(id=>at(id)[1])));
  });
  it('draws each route’s own moves and the book’s places, through the trunk between strata, the same every time',()=>{
    const move=tree.edges.find(edge=>edge.from==='core:jester-leap#0'&&edge.to==='core:jester-leap#1');
    expect(move.kind).toBe('develops');expect(move.label.ru).toBe('объявляет');
    expect(tree.edges.some(edge=>edge.from==='demo:gateway'&&edge.to==='demo:recurrence')).toBe(true);
    const book=tree.edges.find(edge=>edge.from==='work'&&edge.to==='moment');expect(book.kind).toBe('contains');
    const again=zarathustraTree({library,chapters:witnesses.chapters});
    expect([...again.positions]).toEqual([...tree.positions]);
    const parts=tree.names.filter(name=>name.id.startsWith('tree:part:')).map(name=>name.id);
    expect(parts).toEqual(expect.arrayContaining(['tree:part:I','tree:part:II','tree:part:III','tree:part:IV']));
    expect(tree.names.filter(name=>name.kind==='path')).toHaveLength(2);
  });
});
