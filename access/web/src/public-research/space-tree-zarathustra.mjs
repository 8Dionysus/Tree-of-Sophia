// The whole Zarathustra bonsai as one tree in space, for the lens "The tree", by the scheme of Antiquity's tree.
//
// Height is the stratum. The roots are the book: its eighty-one chapters stand round the ground in the order of the
// book, the four parts run under them, and each chapter's root reaches as deep as the number of stars that rest on
// it. The figures of the book stand on the ground where they enter it. Images rise low in the trunk and concepts
// high, each turned toward the chapters it cites. The crown holds the figures' paths through the book: each grows
// above the chapters it reads, a step further out with each move, the paths of one part of the book on one side,
// and the questions stand on the dome above what they ask about. Turned round, the tree shows each part of the book
// with what grew out of it.
//
// The layout is the same every time. A link between strata runs through the trunk.
import witnesses from '../../../../bonsai/zarathustra/witnesses.json' with {type:'json'};
import {TREE_GEOMETRY,treeRegion,treeVia,trunkLines} from './space-tree.mjs';

const bi=(ru,en)=>({ru,en});
const PART_TITLES=Object.freeze({I:bi('Часть первая','First part'),II:bi('Часть вторая','Second part'),III:bi('Часть третья','Third part'),IV:bi('Часть четвёртая','Fourth part')});
const PART_COLORS=Object.freeze({I:'#9fb7d9',II:'#93c2a8',III:'#c9b28a',IV:'#c7a6d6'});
export const ZARATHUSTRA_STRATA=Object.freeze([['crown',bi('Пути и вопросы','Paths and questions')],['concepts',bi('Понятия','Concepts')],['images',bi('Образы','Images')],['figures',bi('Фигуры','The book’s figures')],['texts',bi('Книга','The book')]]);
const PART_GAP=70,CHAPTER=18,FIGURE=30;

/**
 * Lay out the Zarathustra bonsai. `library` is the stand's library with its core attached, `chapters` the book's
 * chapters in order (the witnesses' manifest by default). Every star is a library node, placed under its own id.
 * Returns what the Antiquity tree returns: places and roles, links, lines, names and extent.
 */
export function zarathustraTree({library,chapters=witnesses.chapters}){
  const {T,TOP,ARC,AREA_GAP,R0,RS:STEP,TURN,STRAND,BEAD,point,around,crown,deg,angleOf,wrap,meanAngle,smooth,ring}=TREE_GEOMETRY;
  const positions=new Map(),roles=new Map(),sides=new Map(),edges=[],lines=[],names=[];
  const set=(id,role,p,side=null)=>{if(!id||positions.has(id))return false;positions.set(id,point(p));roles.set(id,role);if(side)sides.set(id,side);return true;};
  const at=id=>positions.get(id);
  const nodes=library?.nodes??[],byId=new Map(nodes.map(node=>[node.id,node]));
  const routes=library?.space?.routes??library?.atlas?.routes??[],core=library?.space?.core??null;
  const index=new Map(chapters.map((chapter,i)=>[chapter.key,i])),partOf=i=>String(chapters[i]?.key??'').split('.')[0];
  // The chapters a star cites, by its own citations and by the grounds of its inquiry.
  const cited=node=>[...(node?.coreCitations??[]).map(cite=>cite.c),...(node?.inquiry?.grounds??[]).flatMap(ground=>(ground.cite??[]).map(cite=>cite.c))].map(key=>index.get(key)).filter(Number.isFinite);
  const entry=node=>{const list=cited(node);return list.length?Math.min(...list):0;};
  const stepsOf=route=>(route.steps??[]).map(step=>step.nodeId??step.id).filter(id=>byId.has(id));

  // The figures are the core's figures of the work; a figure's path is a core route that reads only the book's places.
  const figureIds=new Set((core?.figures??[]).map(figure=>core.materialIds?.[figure.id]).filter(id=>byId.has(id)));
  const paths=routes.filter(route=>{const steps=stepsOf(route);return route.coreChainId&&steps.length>1&&steps.every(id=>byId.get(id).kind==='fragment');});
  const readBy=new Map();for(const route of paths)for(const id of stepsOf(route))if(!readBy.has(id))readBy.set(id,route.id);
  const roleOf=node=>figureIds.has(node.id)?'figure':node.kind==='question'?'question':node.kind==='concept'?'concept':node.kind==='symbol'||node.kind==='character'?'image':node.kind==='work'?'work':readBy.has(node.id)?'step':node.kind==='chapter'?'chapter':'passage';
  const having=role=>nodes.filter(node=>roleOf(node)===role);

  // ── Roots. Each chapter takes its place round the ground in the order of the book; a figure stands on the chapter
  // where it enters the book, and the book's own places hang under the chapter they come from.
  const hanging=new Map(),standing=new Map(),add=(map,i,node)=>{if(!map.has(i))map.set(i,[]);map.get(i).push(node);};
  for(const node of nodes){const role=roleOf(node);if(role==='figure')add(standing,entry(node),node);else if(role==='passage'||role==='chapter')add(hanging,entry(node),node);}
  const weight=chapters.map(()=>0);for(const node of nodes)for(const i of new Set(cited(node)))weight[i]++;
  const slots=[];let s=0;
  chapters.forEach((chapter,i)=>{if(i&&partOf(i)!==partOf(i-1))s+=PART_GAP;const width=Math.max(CHAPTER,(standing.get(i)?.length??0)*FIGURE,(hanging.get(i)?.length??0)*STRAND);slots.push({i,from:s,to:s+width});s+=width;});
  // The book's circle is wide enough to hold its figures apart under a crown of long paths.
  const total=Math.max(1,s),RA=Math.max(760,total/ARC*1.3);
  // The circle turns so that the chapters most stars rest on face the reader at rest.
  const raw=slots.map(slot=>-ARC/2+((slot.from+slot.to)/2/total)*ARC),turn=-meanAngle(raw,slots.map(slot=>1+weight[slot.i]));
  const angle=x=>wrap(-ARC/2+(x/total)*ARC+turn);
  const chapterAngle=slots.map(slot=>angle((slot.from+slot.to)/2));
  let deepest=0;
  for(const slot of slots){
    const phi=chapterAngle[slot.i],own=standing.get(slot.i)??[],width=slot.to-slot.from;
    own.sort((a,b)=>a.id.localeCompare(b.id)).forEach((node,k)=>set(node.id,'figure',around(RA,angle(slot.from+(k+.5)*width/own.length),0),'above'));
    const beads=[];(hanging.get(slot.i)??[]).sort((a,b)=>a.id.localeCompare(b.id)).forEach((node,j)=>{const bead=around(RA+10,phi,62+j*BEAD);if(set(node.id,roleOf(node),bead,'right'))beads.push(bead);});
    // A chapter's root reaches as deep as the number of stars that rest on it.
    const depth=Math.max(30+16*Math.sqrt(weight[slot.i]),beads.length?beads[beads.length-1][1]+18:0);deepest=Math.max(deepest,depth);
    lines.push({tone:'root',points:[...smooth([around(RA,phi,0),around(RA+6,phi,14),around(RA+10,phi,30)],4),point(around(RA+10,phi,depth))]});
  }
  const rootBottom=Math.max(140,deepest,...[...positions.values()].map(p=>p[1]))+40;
  lines.push({tone:'ground',points:ring(RA,0)});
  // The book itself is the taproot under the trunk; its parts run round under the roots.
  for(const work of having('work'))set(work.id,'work',[0,rootBottom-20,0],'below');
  lines.push({tone:'root',points:[[0,0,0],[0,rootBottom-36,0]]});
  const partSpan=new Map();for(const slot of slots){const part=partOf(slot.i),span=partSpan.get(part)??[Infinity,-Infinity];span[0]=Math.min(span[0],slot.from);span[1]=Math.max(span[1],slot.to);partSpan.set(part,span);}
  for(const [part,[from,to]] of partSpan){const a=angle(from),b=angle(to);lines.push({tone:'era',color:PART_COLORS[part],points:ring(RA+50,rootBottom,a,b<a?b+TURN:b,24)});
    names.push({id:`tree:part:${part}`,kind:'area',title:PART_TITLES[part]??bi(part,part),center:point(around(RA+90,angle((from+to)/2),rootBottom+14)),color:PART_COLORS[part]});}

  // ── Trunk. Images low and concepts high; each turns toward the chapters it cites, one height apart from the next.
  const facing=node=>{const list=cited(node);return list.length?meanAngle(list.map(i=>chapterAngle[i])):0;};
  const spiral=(role,radius,from,height)=>having(role).map(node=>({node,phi:facing(node)})).sort((a,b)=>a.phi-b.phi||a.node.id.localeCompare(b.node.id))
    .forEach(({node,phi},j,all)=>set(node.id,role,around(radius(j),phi,from-j*(height/Math.max(1,all.length-1))),Math.sin(phi)<0?'left':'right'));
  spiral('image',()=>92,-100,230);
  spiral('concept',j=>j%2?152:118,-390,440);
  lines.push(...trunkLines());

  // ── Crown. A path grows above the chapters it reads; the paths that read mostly one part of the book share a
  // side of the crown, in the order of the book. A path's first move stands near the trunk and each next one further out.
  const grown=paths.map(route=>{
    const steps=stepsOf(route).filter(id=>readBy.get(id)===route.id),where=steps.flatMap(id=>cited(byId.get(id)));
    const count=new Map();for(const i of where)count.set(partOf(i),(count.get(partOf(i))??0)+1);
    return {route,steps,source:meanAngle(where.map(i=>chapterAngle[i])),part:[...count].sort((a,b)=>b[1]-a[1]||a[0].localeCompare(b[0]))[0]?.[0]??'I'};
  });
  const longest=Math.max(2,...grown.map(g=>g.steps.length)),RS=Math.min(STEP,640/(longest-1));
  const areas=[...new Set(grown.map(g=>g.part))].map(id=>{const own=grown.filter(g=>g.part===id);return {id,own,weight:own.length,source:meanAngle(own.map(g=>g.source))};}).sort((a,b)=>a.source-b.source);
  const free=TURN-AREA_GAP*areas.length;let cursor=0;
  for(const area of areas){area.width=free*area.weight/Math.max(1,grown.length);area.centre=cursor+area.width/2;cursor+=area.width+AREA_GAP;}
  const shift=meanAngle(areas.map(a=>wrap(a.source-a.centre)),areas.map(a=>a.weight));
  for(const area of areas){
    const color=PART_COLORS[area.id]??'#a8b9cc';let a=area.centre-area.width/2+shift;
    area.own.sort((p,q)=>wrap(p.source-area.source)-wrap(q.source-area.source)||p.route.id.localeCompare(q.route.id));
    const joint=crown(140,area.centre+shift,deg(8));
    lines.push({tone:'limb',color,points:smooth([TOP,[joint[0]*.5,(joint[1]+TOP[1])/2-24,joint[2]*.5],joint],6)});
    for(const g of area.own){
      const width=area.width/area.own.length,mid=a+width/2;a+=width;g.mid=wrap(mid);
      const placed=g.steps.map((id,d)=>{const p=crown(R0+d*RS,mid+(d%2?.2:-.2)*width,deg(6+d*7));set(id,'step',p,'right');return point(p);});
      const base=crown(235,mid,deg(5));
      lines.push({tone:'bough',color,points:[...smooth([joint,[(joint[0]+base[0])/2,(joint[1]+base[1])/2-16,(joint[2]+base[2])/2],base,placed[0]],6),...placed.slice(1)]});
      const L=g.steps.length;names.push({id:`tree:path:${g.route.id}`,kind:'path',title:g.route.title,center:point(crown(R0+(L-1)*RS+90,mid,deg(6+(L-1)*7+6))),color});
    }
  }
  // ── The dome. A question stands above the chapters it cites and the stars it is asked of.
  const questions=having('question'),asked=(library?.atlas?.edges??[]).filter(edge=>edge.kind==='questions');
  const want=new Map(questions.map(q=>{const list=[...cited(q).map(i=>chapterAngle[i]),...asked.filter(edge=>edge.to===q.id||edge.from===q.id).map(edge=>at(edge.from===q.id?edge.to:edge.from)).filter(Boolean).map(angleOf)];return [q.id,list.length?meanAngle(list):0];}));
  const dome=[...questions].sort((a,b)=>want.get(a.id)-want.get(b.id)||a.id.localeCompare(b.id)),gap=TURN/Math.max(1,dome.length)*.82,placed=dome.map(q=>want.get(q.id));
  for(let pass=0;pass<4;pass++)for(let i=0;i<placed.length;i++){const next=(i+1)%placed.length,d=wrap(placed[next]-placed[i]);if(d<gap){const push=(gap-d)/2;placed[i]=wrap(placed[i]-push);placed[next]=wrap(placed[next]+push);}}
  const RQ=R0+(longest-1)*RS+200;
  dome.forEach((q,i)=>set(q.id,'question',crown(RQ+(i%2)*50,placed[i],deg(i%2?50:40)),'above'));
  // Whatever the scheme leaves without a place stands with the book's places under the first chapter.
  for(const node of nodes)if(!positions.has(node.id))set(node.id,'passage',around(RA+10,chapterAngle[entry(node)]??0,62+((hanging.get(entry(node))?.length??0)+1)*BEAD),'right');

  // ── Links: every route's own moves, and the book's chapters and places with the book. Between strata through the trunk.
  const seen=new Set();
  const link=(from,to,kind,label)=>{
    if(!from||!to||from===to||!positions.has(from)||!positions.has(to))return;
    const id=`tree:${kind}:${from}>${to}`;if(seen.has(id))return;seen.add(id);
    edges.push({id,from,to,kind,label,via:treeVia(at(from),at(to))});
  };
  for(const route of routes){const steps=stepsOf(route);steps.forEach((id,k)=>{if(!k)return;const move=(route.transitions??[]).find(item=>(item.fromNodeId??item.from)===steps[k-1]&&(item.toNodeId??item.to)===id);link(steps[k-1],id,'develops',move?.label??bi('Следующий шаг пути','The next move of the path'));});}
  for(const work of having('work'))for(const node of nodes)if(roleOf(node)==='chapter'||roleOf(node)==='passage')link(work.id,node.id,'contains',node.kind==='chapter'?bi('Глава книги','A chapter of the book'):bi('Место в книге','A place in the book'));

  // ── Names of the strata, and the tree's extent.
  const all=[...positions.values(),...names.map(name=>name.center)],extent=k=>[Math.min(...all.map(p=>p[k])),Math.max(...all.map(p=>p[k]))];
  const bounds={x:extent(0),y:[extent(1)[0],rootBottom+70],z:extent(2)};
  const band=role=>{const list=[...roles].filter(([,r])=>r===role).map(([id])=>at(id)[1]);return list.length?[Math.min(...list),Math.max(...list)]:null;};
  const top=['question','step'].map(band).filter(Boolean);
  const strata={crown:top.length?[Math.min(...top.map(b=>b[0])),Math.max(...top.map(b=>b[1]))]:null,concepts:band('concept'),images:band('image'),figures:band('figure')?[-24,-24]:null,texts:[40,rootBottom]};
  for(const [id,title] of ZARATHUSTRA_STRATA){const range=strata[id];if(range)names.push({id:`tree:stratum:${id}`,kind:'stratum',title,center:[0,Math.round((range[0]+range[1])/2),0],range});}

  return {positions,roles,sides,edges,lines,names,bounds,rootRadius:RA,regionOf:id=>at(id)?treeRegion(at(id)):null,angleOf:id=>at(id)?angleOf(at(id)):null};
}
