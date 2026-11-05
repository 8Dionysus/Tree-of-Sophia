// The whole bonsai as one tree in space, for the lens "The tree".
//
// Height is the stratum: the texts are the roots under the ground, the words and then the concepts rise in the
// trunk, the branches of inquiry spread in the crown, and their questions stand on its dome. The angle round the
// trunk is where a thought comes from: the authors stand on a circle of time round the foot of the trunk in the
// order of their lives, each one's works hang down from it, and every branch grows up in the crown above the roots
// whose passages it reads, its questions above it on the dome; a word or a concept in the trunk turns toward the
// texts that carry it. The distance from the trunk is how far a thought has gone: a branch's moves stand further
// out with each step. Turned round, the tree shows each tradition with what grew out of it.
//
// The layout is the same every time. Every link the collection states can be drawn on it; a link between strata
// runs through the trunk, so the links of one star gather there the way sap does.

const bi=(ru,en)=>({ru,en});
const AREA_TITLES=Object.freeze({'area-knowledge':bi('Знание','Knowledge'),'area-nature':bi('Природа','Nature'),'area-speech':bi('Речь','Speech'),
  'area-action':bi('Поступок','Action'),'area-good-life':bi('Благая жизнь','The good life'),'area-city':bi('Город','The city'),'area-love':bi('Любовь и образ','Love and image')});
const AREA_COLORS=['#9fb7d9','#93c2a8','#c9b28a','#d39c8a','#c7a6d6','#a8b9cc','#d6a3b6'];
const ERA_COLORS=Object.freeze({classical:'#cdbb8e',hellenistic:'#8fc4b5',roman:'#a9a4d6',late:'#d19c9c'});
const ERA_START=Object.freeze({classical:-500,hellenistic:-330,roman:-30,late:200});
// A school is named by where and when it taught; it stands on the ground of that era.
const SCHOOL_ERA=Object.freeze({academy:'classical',lyceum:'classical',cynics:'classical',garden:'hellenistic','academic-scepticism':'hellenistic',stoa:'hellenistic',skeptics:'hellenistic',platonism:'late'});

/** The zoom (screen pixels per unit of the layout) from which a star's name may stand without being pointed at. */
export const TREE_LABEL_FROM=Object.freeze({era:0,branch:.2,author:.2,concept:.22,question:.42,school:.5,translator:.5,work:.62,version:.8,term:.75,step:.85,passage:1.25});
/** The strata of the tree, top to bottom, as the edge of the map names them. */
export const TREE_STRATA=Object.freeze([['crown',bi('Ветви и вопросы','Branches and questions')],['concepts',bi('Понятия','Concepts')],['words',bi('Слова','Words')],['texts',bi('Тексты','Texts')]]);

const T=900,TOP=[0,-T,0];
const STRAND=32,GAP=30,ERA_GAP=90,EMPTY=58,BEAD=29;
// The circle of time is open at the back, between the last era and the first.
const ARC=300*Math.PI/180,AREA_GAP=6*Math.PI/180,R0=330,RS=150,SX=1.3,SY=.96;
const TURN=2*Math.PI;
const round=v=>Math.round(v*10)/10;
const point=([x,y,z])=>[round(x),round(y),round(z)];
const around=(r,phi,y)=>[Math.sin(phi)*r,y,Math.cos(phi)*r];
// A point of the crown: its distance from the top of the trunk, its angle round the trunk and its rise.
const crown=(r,phi,rise)=>[TOP[0]+Math.sin(phi)*Math.cos(rise)*r*SX,TOP[1]-Math.sin(rise)*r*SY,TOP[2]+Math.cos(phi)*Math.cos(rise)*r*SX];
const deg=d=>d*Math.PI/180;
const angleOf=([x,,z])=>Math.atan2(x,z);
const wrap=a=>{a=(a+Math.PI)%TURN;if(a<0)a+=TURN;return a-Math.PI;};
const meanAngle=(angles,weights=null)=>{let s=0,c=0;angles.forEach((a,i)=>{const w=weights?weights[i]:1;s+=Math.sin(a)*w;c+=Math.cos(a)*w;});return s||c?Math.atan2(s,c):0;};

// A quadratic run through its points, sampled so that a renderer of straight segments draws it smooth.
function smooth(points,per=6){
  if(points.length<3)return points.map(point);
  const out=[points[0]];
  for(let i=1;i<points.length-1;i++){
    const start=out[out.length-1],control=points[i],last=i===points.length-2,end=last?points[i+1]:points[i].map((v,k)=>(v+points[i+1][k])/2);
    for(let s=1;s<=per;s++){const t=s/per,u=1-t;out.push([0,1,2].map(k=>u*u*start[k]+2*u*t*control[k]+t*t*end[k]));}
  }
  return out.map(point);
}
const ring=(r,y,from=-Math.PI,to=Math.PI,steps=96)=>Array.from({length:steps+1},(_,i)=>point(around(r,from+(to-from)*i/steps,y)));

/**
 * Lay out the Antiquity bonsai. `collection` is the bonsai's collection, `chronology` its chronology (for the order
 * of lives) and `materialOf` maps a collection id to the id of its material; a branch's own star is
 * `antiquity:route:<branch id>`. Returns the place and role of every star in space, the links the tree can draw,
 * the lines of its ground, trunk, roots and limbs, its names, and its extent.
 */
export function antiquityTree({collection,chronology=null,materialOf}){
  const positions=new Map(),roles=new Map(),sides=new Map(),edges=[],lines=[],names=[];
  const set=(id,role,p,side=null)=>{if(!id||positions.has(id))return false;positions.set(id,point(p));roles.set(id,role);if(side)sides.set(id,side);return true;};
  const of=legacy=>legacy?materialOf(legacy)??null:null;
  const at=id=>positions.get(id);
  const entities=collection?.entities??[],kind=k=>entities.filter(item=>item.kind===k);
  const authors=kind('author'),works=kind('work'),versions=entities.filter(item=>['translation','original','edition'].includes(item.kind));
  const passages=collection?.passages??[],branches=collection?.branches??[],questions=collection?.questions??[],terms=collection?.terms??[],eras=collection?.eras??[];

  // A passage a branch reads grows up into that branch, the first branch that reads it.
  const readBy=new Map();for(const branch of branches)for(const node of branch.nodes??[])if(!readBy.has(node.id))readBy.set(node.id,branch.id);

  // ── Roots. The authors stand round the foot of the trunk in the order of their lives; under each hang its works,
  // the longest in the middle, and along each work its passages that no branch reads.
  const lifeStart=id=>{const situation=chronology?.situations?.[chronology?.lives?.[id]],year=parseInt(situation?.dating?.interval?.start,10);return Number.isFinite(year)?year:null;};
  const ordered=[...authors].sort((a,b)=>(lifeStart(a.id)??ERA_START[a.period]??0)-(lifeStart(b.id)??ERA_START[b.period]??0));
  const rooted=work=>passages.filter(p=>p.workId===work.id&&!readBy.has(p.id));
  const slots=[];let s=0,previous=null;
  for(const author of ordered){
    if(previous&&author.period!==previous)s+=ERA_GAP;previous=author.period;
    const own=works.filter(w=>w.author===author.id).sort((a,b)=>rooted(b).length-rooted(a).length),line=[];
    own.forEach((w,i)=>{if(i%2)line.push(w);else line.unshift(w);});
    // A work's other witnesses — its original, its translations and their editions — hang beside it.
    const kin=versions.filter(v=>own.some(w=>w.id===v.work)).sort((a,b)=>['original','translation','edition'].indexOf(a.kind)-['original','translation','edition'].indexOf(b.kind));
    const items=[...line,...kin],width=Math.max(EMPTY,items.length*STRAND);
    slots.push({author,from:s,to:s+width,items,era:author.period});s+=width+GAP;
  }
  const total=Math.max(1,s-GAP),RA=Math.max(560,total/ARC*1.3);
  // The circle turns so that the heart of the material faces the reader at rest.
  const weightOf=slot=>slot.items.reduce((n,w)=>n+1+rooted(w).length+passages.filter(p=>p.workId===w.id).length,0);
  const raw=slots.map(slot=>-ARC/2+((slot.from+slot.to)/2/total)*ARC),turn=-meanAngle(raw,slots.map(weightOf));
  const angle=at=>wrap(-ARC/2+(at/total)*ARC+turn);
  const strands=new Map(),eraSpan={};
  for(const slot of slots){
    const centre=angle((slot.from+slot.to)/2),foot=around(RA,centre,0);
    set(of(slot.author.id),'author',foot,'above');
    const span=eraSpan[slot.era]??=[Infinity,-Infinity];span[0]=Math.min(span[0],slot.from);span[1]=Math.max(span[1],slot.to);
    const left=slot.from+(slot.to-slot.from-slot.items.length*STRAND)/2+STRAND/2;
    slot.items.forEach((w,i)=>{
      const phi=angle(left+i*STRAND),version=w.kind!=='work',top=around(RA+16,phi,62);
      set(of(w.id),version?'version':'work',top,'above');
      const beads=[];(version?[]:rooted(w)).forEach((p,j)=>{const bead=around(RA+16+(j+1)*7,phi,96+j*BEAD);if(set(of(p.id),'passage',bead,'right'))beads.push(bead);});
      const bottom=beads.length?beads[beads.length-1]:around(RA+16,phi,version?78:86);
      lines.push({tone:'root',points:[...smooth([foot,[foot[0],34,foot[2]],around(RA+10,phi,28),top],5),...beads.map(point),...(beads.length?[]:[point(bottom)])]});
      strands.set(w.id,{phi,bottom});
    });
  }
  const rootBottom=Math.max(140,...[...positions.values()].map(p=>p[1]))+40;
  // Translators stand under the translations they made.
  for(const translator of kind('translator')){const made=versions.find(v=>v.translator===translator.id),strand=made&&strands.get(made.id);if(strand)set(of(translator.id),'translator',[strand.bottom[0]*1.04,strand.bottom[1]+40,strand.bottom[2]*1.04],'right');}
  // The ground is a circle; under the roots the eras run round it with their stars.
  lines.push({tone:'ground',points:ring(RA,0)});
  for(const era of eras){const span=eraSpan[era.period];if(!span)continue;const from=angle(span[0]),to=angle(span[1]),mid=angle((span[0]+span[1])/2);
    lines.push({tone:'era',color:ERA_COLORS[era.period],points:ring(RA+50,rootBottom,from,to<from?to+TURN:to,24)});set(of(era.id),'era',around(RA+70,mid,rootBottom+6),'below');}
  // Schools stand just above the ground, in their era, a little inside the circle.
  for(const era of Object.keys(eraSpan)){const own=kind('school').filter(item=>(SCHOOL_ERA[item.id]??'hellenistic')===era),span=eraSpan[era];own.forEach((item,i)=>set(of(item.id),'school',around(RA-60,angle(span[0]+(span[1]-span[0])*(i+1)/(own.length+1)),-34),'above'));}

  // ── Trunk. Words low and concepts high; each turns toward the texts that carry it, one height apart from the next.
  const facing=list=>{const angles=list.map(id=>at(id)).filter(Boolean).map(angleOf);return angles.length?meanAngle(angles):0;};
  const wordFacing=term=>facing([...new Set((term.uses??[]).map(use=>use.work))].map(of));
  [...terms].map(term=>({term,phi:wordFacing(term)})).sort((a,b)=>a.phi-b.phi).forEach(({term,phi},j,all)=>set(of(`lexical:${term.id}`),'term',around(88,phi,-92-j*(250/Math.max(1,all.length-1))),Math.sin(phi)<0?'left':'right'));
  const conceptTexts=id=>passages.filter(p=>(p.concepts??[]).includes(id)).map(p=>of(p.id));
  // ── The crown comes before the concepts' turn is final: a concept faces its texts and its questions together.
  const workAngle=new Map(works.map(w=>[w.id,at(of(w.id))?angleOf(at(of(w.id))):0]));
  const passageWork=new Map(passages.map(p=>[p.id,p.workId]));
  const grown=branches.map(branch=>{
    const neighbours=new Map();for(const edge of branch.edges??[]){for(const [a,b] of [[edge.from,edge.to],[edge.to,edge.from]]){if(!neighbours.has(a))neighbours.set(a,[]);neighbours.get(a).push(b);}}
    const depth=new Map([[branch.root,0]]),queue=[branch.root];
    while(queue.length){const id=queue.shift();for(const next of neighbours.get(id)??[])if(!depth.has(next)){depth.set(next,depth.get(id)+1);queue.push(next);}}
    const levels=[];for(const node of branch.nodes??[]){const d=depth.get(node.id)??1;(levels[d]??=[]).push(node.id);}
    const sources=(branch.nodes??[]).map(node=>workAngle.get(passageWork.get(node.id))).filter(Number.isFinite);
    return {branch,levels:Array.from(levels,level=>level??[]),weight:Math.max(2.2,...Array.from(levels,level=>level?.length??0)),source:meanAngle(sources)};
  });
  // Each area of inquiry takes the side of the crown above the roots its branches read, in that order round the trunk.
  const areaIds=[...new Set(branches.map(b=>b.area))];
  const areas=areaIds.map((id,index)=>{const own=grown.filter(g=>g.branch.area===id);return {id,index,own,weight:own.reduce((n,g)=>n+g.weight,0),source:meanAngle(own.map(g=>g.source),own.map(g=>g.branch.nodes?.length??1))};}).sort((a,b)=>a.source-b.source);
  const free=TURN-AREA_GAP*areas.length,weightTotal=areas.reduce((n,a)=>n+a.weight,0);
  let cursor=0;for(const area of areas){area.width=free*area.weight/weightTotal;area.centre=cursor+area.width/2;cursor+=area.width+AREA_GAP;}
  const shift=meanAngle(areas.map(a=>wrap(a.source-a.centre)),areas.map(a=>a.weight));
  const areaPlaces=[];
  for(const area of areas){
    const color=AREA_COLORS[area.index%AREA_COLORS.length];let a=area.centre-area.width/2+shift;
    area.own.sort((p,q)=>wrap(p.source-area.source)-wrap(q.source-area.source));
    const joint=crown(140,area.centre+shift,deg(8));
    for(const g of area.own){
      const width=area.width*g.weight/area.weight,a0=a,a1=a+width,mid=a+width/2;a=a1;g.mid=wrap(mid);
      g.levels.forEach((level,d)=>{const r=R0+d*RS,rise=deg(6+d*7);level.forEach((id,i)=>{const t=level.length===1?.5:i/(level.length-1),phi=level.length===1?mid:a0+(a1-a0)*(.16+.68*t);if(readBy.get(id)===g.branch.id)set(of(id),'step',crown(r,phi,rise),'right');});});
      const L=g.levels.length;set(`antiquity:route:${g.branch.id}`,'branch',crown(R0+(L-1)*RS+100,mid,deg(6+(L-1)*7+6)),'above');
      const base=crown(235,mid,deg(5)),first=at(of(g.branch.root))??crown(R0,mid,deg(6));
      lines.push({tone:'limb',color,points:smooth([TOP,[joint[0]*.5,(joint[1]+TOP[1])/2-24,joint[2]*.5],joint],6)},
        {tone:'bough',color,points:smooth([joint,[(joint[0]+base[0])/2,(joint[1]+base[1])/2-16,(joint[2]+base[2])/2],base,first],6)});
    }
    areaPlaces.push({id:area.id,title:AREA_TITLES[area.id]??bi(area.id,area.id),branches:area.own.length,center:crown(R0+4*RS+150,area.centre+shift,deg(16)),color});
  }
  // ── The dome. Each question stands above the branches that ask it, else beside its kin by a shared concept.
  const askers=new Map();for(const g of grown)for(const q of g.branch.questions??[]){const id=typeof q==='string'?q:q.ru;if(!askers.has(id))askers.set(id,[]);askers.get(id).push(g.mid);}
  const want=new Map();for(const q of questions){const list=askers.get(q.id);if(list?.length)want.set(q.id,meanAngle(list));}
  for(const q of questions)if(!want.has(q.id)){const mine=new Set(q.concepts??[]),kin=questions.filter(o=>want.has(o.id)&&(o.concepts??[]).some(c=>mine.has(c)));want.set(q.id,kin.length?meanAngle(kin.map(o=>want.get(o.id))):0);}
  const dome=[...questions].sort((a,b)=>want.get(a.id)-want.get(b.id)||a.id.localeCompare(b.id)),gap=TURN/Math.max(1,dome.length)*.82;
  const placed=dome.map(q=>want.get(q.id));
  for(let pass=0;pass<4;pass++)for(let i=0;i<placed.length;i++){const next=(i+1)%placed.length,d=wrap(placed[next]-placed[i]);if(d<gap){const push=(gap-d)/2;placed[i]=wrap(placed[i]-push);placed[next]=wrap(placed[next]+push);}}
  const RQ=R0+4*RS+200;
  dome.forEach((q,i)=>set(of(q.id),'question',crown(RQ+(i%2)*50,placed[i],deg(i%2?50:40)),'above'));
  // A concept faces the texts that carry it and the questions that ask after it.
  const conceptQuestions=id=>questions.filter(q=>(q.concepts??[]).includes(id)).map(q=>of(q.id));
  [...kind('concept')].map(concept=>({concept,phi:facing([...conceptTexts(concept.id),...conceptQuestions(concept.id)])})).sort((a,b)=>a.phi-b.phi)
    .forEach(({concept,phi},j,all)=>set(of(concept.id),'concept',around(122,phi,-430-j*(400/Math.max(1,all.length-1))),Math.sin(phi)<0?'left':'right'));
  // The trunk: its outline at four sides and its rings at the ground and under the crown.
  const width=y=>y>-260?178-(178-122)*(-y/260):y>-620?122-4*((-y-260)/360):y>-T+30?118+32*((-y-620)/(T-650)):150+55*((-y-T+30)/90);
  for(const phi of [0,Math.PI/2,Math.PI,-Math.PI/2])lines.push({tone:'trunk',points:smooth([0,-260,-620,-T+30,-T-60].map(y=>around(width(y),phi,y)),6)});
  lines.push({tone:'trunk',points:ring(178,0,-Math.PI,Math.PI,48)},{tone:'trunk',points:ring(150,-T+30,-Math.PI,Math.PI,48)});

  // ── Links. Every one the collection states; between strata through the trunk.
  const regionOf=([x,y,z])=>y>-60?'roots':Math.hypot(x,z)<200&&y>-T+40?'trunk':'crown';
  const toward=([x,y,z],k,height)=>[x*k,height,z*k];
  const via=(a,b)=>{
    const ra=regionOf(a),rb=regionOf(b);
    if(ra===rb){if(ra!=='roots')return [];return smooth([a,toward(a.map((v,k)=>(v+b[k])/2),.86,Math.min(a[1],b[1])-34),b],5).slice(1,-1);}
    const low=ra==='roots'?a:rb==='roots'?b:null,high=low===a?b:a;
    let path;
    if(low&&regionOf(high)==='crown')path=[low,toward(low,.3,-50),[0,-T*.5,0],[0,-T-40,0],high];
    else if(low)path=[low,toward(low,.32,-46),high];
    else{const trunk=ra==='trunk'?a:b,top=trunk===a?b:a;path=[trunk,toward(trunk,.5,-T-30),top];}
    if(path[0]!==a)path.reverse();
    return smooth(path,6).slice(1,-1);
  };
  const seen=new Set();
  const link=(from,to,edgeKind,label,extra={})=>{
    if(!from||!to||from===to||!positions.has(from)||!positions.has(to))return;
    const id=`tree:${edgeKind}:${from}>${to}`;if(seen.has(id))return;seen.add(id);
    edges.push({id,from,to,kind:edgeKind,label,via:via(at(from),at(to)),...extra});
  };
  for(const w of works)link(of(w.author),of(w.id),'contains',bi('Произведение автора','A work of the author'));
  for(const v of versions){link(of(v.work),of(v.id),'translates',bi(v.kind==='original'?'Подлинник':v.kind==='edition'?'Издание':'Перевод',v.kind==='original'?'The original':v.kind==='edition'?'An edition':'A translation'));
    if(v.translator)link(of(v.translator),of(v.id),'relates',bi('Переводчик','The translator'));if(v.translation)link(of(v.translation),of(v.id),'contains',bi('Издание перевода','An edition of the translation'));}
  for(const p of passages){
    link(of(p.workId),of(p.id),'contains',bi('Место в произведении','A passage of the work'));
    for(const concept of p.concepts??[])link(of(p.id),of(concept),'relates',bi('Понятие места','A concept of the passage'));
    for(const who of p.about??[])link(of(p.id),of(who),'relates',bi('О ком это место','Whom the passage is about'));
  }
  for(const q of questions){
    for(const concept of q.concepts??[])link(of(q.id),of(concept),'relates',bi('Понятие вопроса','A concept of the question'));
    for(const position of q.positions??[])link(of(position.passage),of(q.id),'supports',position.voice??bi('Ответ','An answer'),{voice:position.title??null});
  }
  for(const g of grown){
    const own=`antiquity:route:${g.branch.id}`;
    link(own,of(g.branch.root),'develops',bi('Начать с этого места','Begin here'));
    for(const q of g.branch.questions??[])link(own,of(typeof q==='string'?q:q.ru),'questions',bi('Вопрос ветви','A question of the branch'),{rest:.14});
  }
  for(const term of terms){
    for(const other of term.related??[]){const [a,b]=[term.id,other].sort();link(of(`lexical:${a}`),of(`lexical:${b}`),'echoes',bi('Родственное слово','A related word'));}
    for(const work of new Set((term.uses??[]).map(use=>use.work)))link(of(`lexical:${term.id}`),of(work),'relates',bi('Слово этого текста','A word of this text'));
  }

  // ── Names: the areas round the crown and the strata of the tree, and the tree's extent.
  const all=[...positions.values(),...areaPlaces.map(a=>a.center)],extent=k=>[Math.min(...all.map(p=>p[k])),Math.max(...all.map(p=>p[k]))];
  const bounds={x:extent(0),y:[extent(1)[0],rootBottom+70],z:extent(2)};
  const band=role=>{const list=[...roles].filter(([,r])=>r===role).map(([id])=>at(id)[1]);return list.length?[Math.min(...list),Math.max(...list)]:null;};
  const top=['question','branch','step'].map(band).filter(Boolean);
  const strata={crown:[Math.min(...top.map(b=>b[0])),Math.max(...top.map(b=>b[1]))],concepts:band('concept'),words:band('term'),texts:[-34,rootBottom]};
  for(const area of areaPlaces)names.push({id:`tree:area:${area.id}`,kind:'area',title:area.title,count:area.branches,center:point(area.center),color:area.color});
  for(const [id,title] of TREE_STRATA){const range=strata[id];if(range)names.push({id:`tree:stratum:${id}`,kind:'stratum',title,center:[0,round((range[0]+range[1])/2),0],range});}

  return {positions,roles,sides,edges,lines,names,bounds,rootRadius:RA,regionOf:id=>at(id)?regionOf(at(id)):null,angleOf:id=>at(id)?angleOf(at(id)):null};
}
