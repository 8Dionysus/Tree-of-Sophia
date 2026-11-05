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
const ellipse=([x,y])=>Math.hypot(x/1.22,(y+900)/.86);

describe('the whole Antiquity bonsai as one tree',()=>{
  it('gives every star of the bonsai a place of its own',()=>{
    const expected=[...collection.entities.map(e=>e.id),...collection.passages.map(p=>p.id),...collection.questions.map(q=>q.id),...collection.eras.map(e=>e.id),...collection.terms.map(t=>`lexical:${t.id}`)].map(materialOf);
    for(const id of expected)expect(at(id),id).toBeTruthy();
    for(const branch of collection.branches)expect(at(`antiquity:route:${branch.id}`),branch.id).toBeTruthy();
    expect(tree.positions.size).toBe(expected.length+collection.branches.length);
    const places=new Set([...tree.positions.values()].map(([x,y])=>`${Math.round(x)}:${Math.round(y)}`));
    expect(places.size).toBe(tree.positions.size);
  });
  it('raises a passage a branch reads into the crown and keeps every other one on its work in the roots',()=>{
    const read=new Set(collection.branches.flatMap(b=>b.nodes.map(n=>n.id)));
    for(const p of collection.passages){
      const id=materialOf(p.id),[x,y]=at(id),[wx,wy]=at(materialOf(p.workId));
      if(read.has(p.id)){expect(tree.roles.get(id)).toBe('step');expect(tree.regionOf(id)).toBe('crown');}
      else{expect(tree.roles.get(id)).toBe('passage');expect(y).toBeGreaterThan(wy);expect(Math.abs(x-wx)).toBeLessThan(40);}
    }
  });
  it('stands the authors on the ground in the order of their lives',()=>{
    const start=id=>parseInt(chronology.situations[chronology.lives[id]]?.dating?.interval?.start,10);
    const dated=collection.entities.filter(e=>e.kind==='author'&&Number.isFinite(start(e.id))).sort((a,b)=>at(materialOf(a.id))[0]-at(materialOf(b.id))[0]);
    for(const a of dated)expect(at(materialOf(a.id))[1]).toBe(0);
    for(let i=1;i<dated.length;i++)expect(start(dated[i].id)).toBeGreaterThanOrEqual(start(dated[i-1].id));
  });
  it('keeps words low in the trunk, concepts above them and questions on the rim of the crown',()=>{
    const role=r=>[...tree.roles].filter(([,v])=>v===r).map(([id])=>at(id));
    const words=role('term'),concepts=role('concept'),questions=role('question'),steps=role('step');
    expect(words).toHaveLength(collection.terms.length);expect(questions).toHaveLength(collection.questions.length);
    expect(Math.max(...concepts.map(p=>p[1]))).toBeLessThan(Math.min(...words.map(p=>p[1])));
    for(const p of [...words,...concepts])expect(Math.abs(p[0])).toBeLessThan(160);
    expect(Math.min(...questions.map(ellipse))).toBeGreaterThan(Math.max(...steps.map(ellipse)));
  });
  it('draws only links the collection states, between placed stars, through the trunk when they cross strata',()=>{
    expect(tree.edges.length).toBeGreaterThan(1000);
    const ids=new Set();
    for(const edge of tree.edges){expect(at(edge.from)).toBeTruthy();expect(at(edge.to)).toBeTruthy();expect(ids.has(edge.id)).toBe(false);ids.add(edge.id);}
    const answers=tree.edges.filter(e=>e.kind==='supports');
    expect(answers).toHaveLength(new Set(collection.questions.flatMap(q=>q.positions.map(p=>`${p.passage}>${q.id}`))).size);
    const crossing=tree.edges.find(e=>tree.regionOf(e.from)==='roots'&&tree.regionOf(e.to)==='crown');
    expect(crossing.via.some(([x,y])=>Math.abs(x)<30&&y<-300)).toBe(true);
  });
  it('is the same tree every time and holds every star inside its bounds',()=>{
    const again=antiquityTree({collection,chronology,materialOf});
    expect([...again.positions]).toEqual([...tree.positions]);expect(again.edges.map(e=>e.id)).toEqual(tree.edges.map(e=>e.id));
    for(const [x,y] of tree.positions.values()){expect(x).toBeGreaterThanOrEqual(tree.bounds.x[0]);expect(x).toBeLessThanOrEqual(tree.bounds.x[1]);expect(y).toBeGreaterThanOrEqual(tree.bounds.y[0]);expect(y).toBeLessThanOrEqual(tree.bounds.y[1]);}
    expect(new Set(tree.names.filter(n=>n.kind==='area').map(n=>n.id)).size).toBe(new Set(collection.branches.map(b=>b.area)).size);
    for(const role of new Set(tree.roles.values()))expect(TREE_LABEL_FROM[role],role).toBeTypeOf('number');
  });
});
