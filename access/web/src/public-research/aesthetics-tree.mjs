import {readChronology} from '../../../../bonsai/lib/chronology.mjs';
import {trunkLines,treeRegion} from './space-tree.mjs';

// The four groups occupy the crown, concepts the trunk, and works the roots in life order.
export function aestheticsTree({library,field,catalog,chronology}){
  const positions=new Map(),roles=new Map(),sides=new Map(),lines=trunkLines(),names=[],notes=new Map();
  const clock=readChronology(chronology),span=id=>clock.span(id)?.from??Infinity;
  const set=(id,role,p)=>{positions.set(id,p);roles.set(id,role);};
  const groups=['A','B','C','D'],areas=field.areas.filter(area=>library.nodes.some(node=>node.id===`aesthetics:route:${area.id}`));
  for(const [g,group] of groups.entries()){
    const own=areas.filter(area=>area.group===group),centerX=(g-1.5)*440;
    for(const [i,area] of own.entries()){
      // Four adjacent branches spread across a dome, so none is hidden behind another at rest.
      const u=own.length>1?i/(own.length-1)*2-1:0,x=centerX+u*190;
      const y=-1160-260*(1-Math.pow(x/900,2))-85*(1-u*u),z=(i%2?1:-1)*55;
      set(`aesthetics:route:${area.id}`,'branch',[x,y,z]);
      const readings=field.conceptions.filter(item=>item.areas[0]===area.id);
      for(const [j,item] of readings.entries()){
        const a=j*2.399963,r=30+14*Math.sqrt(j);
        set(`aesthetics:reading:${item.id}`,'step',[x+Math.cos(a)*r,y+65+Math.sin(a)*r,z+100+(j%3)*25]);
      }
    }
    names.push({id:`group-${group}`,title:{ru:`Группа ${group}`,en:`Group ${group}`},center:[centerX,-1580,0],kind:'path'});
  }
  const concepts=library.nodes.filter(node=>node.kind==='concept');
  for(const [i,node] of concepts.entries()){const angle=i*2.399963,r=75+(i%3)*30;set(node.id,'concept',[Math.sin(angle)*r,-120-(i/Math.max(1,concepts.length))*760,Math.cos(angle)*r]);}
  const works=Object.entries(catalog.works).sort(([,a],[,b])=>span(chronology.lives[a.agents?.[0]])-span(chronology.lives[b.agents?.[0]]));
  for(const [i,[id,work]] of works.entries()){
    const angle=2*Math.PI*i/Math.max(1,works.length),point=r=>[Math.sin(angle)*r,120+Math.floor(i/20)*45,Math.cos(angle)*r];
    set(`aesthetics:catalog:${id}`,'work',point(420));
    for(const [j,node] of library.nodes.filter(node=>node.kind==='fragment'&&node.workId===id).entries())set(node.id,'passage',[Math.sin(angle+(j%5-2)*.035)*(480+Math.floor(j/5)*30),180+Math.floor(j/5)*32,Math.cos(angle+(j%5-2)*.035)*(480+Math.floor(j/5)*30)]);
    for(const author of work.agents??[]){const aid=`aesthetics:catalog:${author}`;if(!positions.has(aid))set(aid,'author',point(300));}
  }
  for(const [i,node] of library.nodes.entries())if(!positions.has(node.id))set(node.id,'step',[(i%20)*35-350,-1020,Math.floor(i/20)*30]);
  const edges=library.atlas.edges.map(({from,to,kind})=>({from,to,kind}));
  return {positions,roles,sides,edges,lines,names,notes,bounds:{x:[-1000,1000],y:[-1650,500],z:[-1000,1000]},rootRadius:600,regionOf:id=>positions.has(id)?treeRegion(positions.get(id)):null,angleOf:id=>{const p=positions.get(id);return p?Math.atan2(p[0],p[2]):null;}};
}

// The source lens shares the colonnade's work, place and reading shape.
export function aestheticsSourceGroups({library,chronology}){
  const clock=readChronology(chronology),year=id=>clock.span(id)?.from??Infinity;
  const works=library.nodes.filter(node=>node.kind==='work').sort((a,b)=>year(a.dating)-year(b.dating));
  const authors=new Map();
  for(const work of works){
    const id=work.author?`aesthetics:catalog:${work.author}`:null;
    if(!authors.has(id))authors.set(id,[]);
    const own=library.nodes.filter(node=>node.workId===work.legacyId);
    authors.get(id).push({id:work.id,up:own.filter(node=>node.kind==='fragment').map(node=>node.id),front:own.filter(node=>node.kind==='interpretation').map(node=>node.id)});
  }
  return [...authors].map(([id,works])=>({id,works,at:year(library.nodes.find(node=>node.id===id)?.life)})).sort((a,b)=>a.at-b.at||String(a.id).localeCompare(String(b.id)));
}
