import {describe,expect,it} from 'vitest';
import collection from '../../../../bonsai/antiquity/collection.json' with {type:'json'};
import chronology from '../../../../bonsai/antiquity/chronology.json' with {type:'json'};
import {antiquityTree,TREE_LABEL_FROM} from './space-tree.mjs';

// The collection's material ids, as the stand binds them.
const PUBLIC={apology:'tos.work.plato.apology',meno:'tos.work.plato.meno',ethics:'tos.work.aristotle.nicomachean-ethics'};
const passages=new Set(collection.passages.map(p=>p.id));
const materialOf=id=>passages.has(id)?`antiquity:reading:${id}`:PUBLIC[id]?`antiquity:work:${PUBLIC[id]}`:`antiquity:catalog:${id}`;
const tree=antiquityTree({collection,chronology,materialOf});
const at=id=>tree.positions.get(id);
const reach=([x,,z])=>Math.hypot(x,z);
const fromTop=([x,y,z])=>Math.hypot(x,y+900,z);
const apart=(a,b)=>{const d=Math.abs(a-b)%(2*Math.PI);return Math.min(d,2*Math.PI-d);};
const meanAngle=list=>Math.atan2(list.reduce((s,a)=>s+Math.sin(a),0),list.reduce((s,a)=>s+Math.cos(a),0));

describe('the whole Antiquity bonsai as one tree in space',()=>{
  it('gives every star of the bonsai a place of its own, in three dimensions',()=>{
    const expected=[...collection.entities.map(e=>e.id),...collection.passages.map(p=>p.id),...collection.questions.map(q=>q.id),...collection.eras.map(e=>e.id),...collection.terms.map(t=>`lexical:${t.id}`)].map(materialOf);
    for(const id of expected)expect(at(id),id).toBeTruthy();
    for(const branch of collection.branches)expect(at(`antiquity:route:${branch.id}`),branch.id).toBeTruthy();
    expect(tree.positions.size).toBe(expected.length+collection.branches.length);
    const places=new Set([...tree.positions.values()].map(p=>p.map(Math.round).join(':')));
    expect(places.size).toBe(tree.positions.size);
    // The tree has depth: its stars spread before and behind the trunk as widely as to its sides.
    const zs=[...tree.positions.values()].map(p=>p[2]),xs=[...tree.positions.values()].map(p=>p[0]);
    expect(Math.max(...zs)-Math.min(...zs)).toBeGreaterThan((Math.max(...xs)-Math.min(...xs))*.6);
  });
  it('raises a passage a branch reads into the crown and hangs every other one under its work',()=>{
    const read=new Set(collection.branches.flatMap(b=>b.nodes.map(n=>n.id)));
    for(const p of collection.passages){
      const id=materialOf(p.id),own=at(id),work=at(materialOf(p.workId));
      if(read.has(p.id)){expect(tree.roles.get(id)).toBe('step');expect(tree.regionOf(id)).toBe('crown');}
      else{expect(tree.roles.get(id)).toBe('passage');expect(own[1]).toBeGreaterThan(work[1]);expect(apart(tree.angleOf(id),tree.angleOf(materialOf(p.workId)))).toBeLessThan(.01);}
    }
  });
  it('stands the authors on a circle of time round the foot of the trunk, in the order of their lives',()=>{
    const start=id=>parseInt(chronology.situations[chronology.lives[id]]?.dating?.interval?.start,10);
    const dated=collection.entities.filter(e=>e.kind==='author'&&Number.isFinite(start(e.id)));
    for(const a of dated){const p=at(materialOf(a.id));expect(p[1]).toBe(0);expect(Math.abs(reach(p)-tree.rootRadius)).toBeLessThan(1);}
    // Going round the circle from where it opens, its widest gap, the lives follow one another.
    const angle=a=>tree.angleOf(materialOf(a.id)),sorted=[...dated].sort((a,b)=>angle(a)-angle(b));
    const gaps=sorted.map((a,i)=>((angle(sorted[(i+1)%sorted.length])-angle(a))%(2*Math.PI)+2*Math.PI)%(2*Math.PI));
    const open=gaps.indexOf(Math.max(...gaps)),byCircle=sorted.map((_,i)=>sorted[(open+1+i)%sorted.length]);
    for(let i=1;i<byCircle.length;i++)expect(start(byCircle[i].id)).toBeGreaterThanOrEqual(start(byCircle[i-1].id));
  });
  it('turns the areas of the crown in the order of the roots they read, and stands the questions above the branches',()=>{
    const sourceOf=branch=>meanAngle(branch.nodes.map(n=>collection.passages.find(p=>p.id===n.id)?.workId).filter(Boolean).map(w=>tree.angleOf(materialOf(w))));
    const areas=[...new Set(collection.branches.map(b=>b.area))].map(id=>{const own=collection.branches.filter(b=>b.area===id);
      return {source:meanAngle(own.map(sourceOf)),place:meanAngle(own.map(b=>tree.angleOf(`antiquity:route:${b.id}`)))};});
    // Round the crown the areas follow one another as their roots do round the ground.
    const cyclic=list=>{const order=list.map((_,i)=>i).sort((a,b)=>list[a]-list[b]);const start=order.indexOf(0);return [...order.slice(start),...order.slice(0,start)];};
    expect(cyclic(areas.map(a=>a.place))).toEqual(cyclic(areas.map(a=>a.source)));
    const gaps=collection.branches.map(b=>apart(tree.angleOf(`antiquity:route:${b.id}`),sourceOf(b))).sort((a,b)=>a-b);
    expect(gaps[Math.floor(gaps.length/2)]).toBeLessThan(Math.PI/2);
    const steps=[...tree.roles].filter(([,r])=>r==='step').map(([id])=>at(id)),asked=[...tree.roles].filter(([,r])=>r==='question').map(([id])=>at(id));
    expect(Math.min(...asked.map(fromTop))).toBeGreaterThan(Math.max(...steps.map(fromTop)));
    expect(Math.max(...asked.map(p=>p[1]))).toBeLessThan(-900);
  });
  it('keeps words low in the trunk and concepts above them, inside the trunk',()=>{
    const role=r=>[...tree.roles].filter(([,v])=>v===r).map(([id])=>at(id));
    const words=role('term'),concepts=role('concept');
    expect(words).toHaveLength(collection.terms.length);
    expect(Math.max(...concepts.map(p=>p[1]))).toBeLessThan(Math.min(...words.map(p=>p[1])));
    for(const p of [...words,...concepts])expect(reach(p)).toBeLessThan(160);
  });
  it('draws only links the collection states, between placed stars, through the trunk when they cross strata',()=>{
    expect(tree.edges.length).toBeGreaterThan(1000);
    const ids=new Set();
    for(const edge of tree.edges){expect(at(edge.from)).toBeTruthy();expect(at(edge.to)).toBeTruthy();expect(ids.has(edge.id)).toBe(false);ids.add(edge.id);}
    const answers=tree.edges.filter(e=>e.kind==='supports');
    expect(answers).toHaveLength(new Set(collection.questions.flatMap(q=>q.positions.map(p=>`${p.passage}>${q.id}`))).size);
    const crossing=tree.edges.find(e=>tree.regionOf(e.from)==='roots'&&tree.regionOf(e.to)==='crown');
    expect(crossing.via.some(p=>reach(p)<30&&p[1]<-300)).toBe(true);
  });
  it('is the same tree every time and holds every star inside its bounds',()=>{
    const again=antiquityTree({collection,chronology,materialOf});
    expect([...again.positions]).toEqual([...tree.positions]);expect(again.edges.map(e=>e.id)).toEqual(tree.edges.map(e=>e.id));
    for(const p of tree.positions.values())['x','y','z'].forEach((axis,k)=>{expect(p[k]).toBeGreaterThanOrEqual(tree.bounds[axis][0]);expect(p[k]).toBeLessThanOrEqual(tree.bounds[axis][1]);});
    expect(new Set(tree.names.filter(n=>n.kind==='area').map(n=>n.id)).size).toBe(new Set(collection.branches.map(b=>b.area)).size);
    for(const role of new Set(tree.roles.values()))expect(TREE_LABEL_FROM[role],role).toBeTypeOf('number');
  });
});
