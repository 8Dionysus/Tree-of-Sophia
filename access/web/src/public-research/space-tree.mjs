// The whole bonsai as one tree, for the lens "The tree".
//
// Its roots are the texts: the authors stand on the ground in the order of their lives, each one's works hang below
// them, and along each work hang its passages that no branch reads. The trunk holds the words low, near the texts,
// and the concepts high, near the questions; each stands on the side where its texts lie. The crown fans out into
// the areas of inquiry. Each branch grows out of the trunk by its own moves, the passages it reads, depth by depth,
// and ends in its name; the questions stand along the rim of the crown, by the branches that ask them.
//
// The layout measures nothing: a star's place says what it is and what it belongs to, and it is the same every time.
// Every link of a star can be drawn on it. A link between strata runs through the trunk, so the links of one star
// gather there the way sap does.

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
/** The strata of the tree, top to bottom, as the left of the map names them. */
export const TREE_STRATA=Object.freeze([['crown',bi('Ветви и вопросы','Branches and questions')],['concepts',bi('Понятия','Concepts')],['words',bi('Слова','Words')],['texts',bi('Тексты','Texts')]]);

const T=900,C={x:0,y:-T};
const STRAND=32,GAP=30,ERA_GAP=90,EMPTY=58,BEAD=29;
const A0=-168*Math.PI/180,A1=-12*Math.PI/180,AREA_GAP=3.2*Math.PI/180,R0=330,RS=128,SX=1.22,SY=.86;
const polar=(r,a)=>[C.x+Math.cos(a)*r*SX,C.y+Math.sin(a)*r*SY];
const round=v=>Math.round(v*10)/10;

// A quadratic run through its points, sampled so that a renderer of straight segments draws it smooth.
function smooth(points,per=6){
  if(points.length<3)return points.map(([x,y])=>[round(x),round(y)]);
  const out=[[points[0][0],points[0][1]]];
  for(let i=1;i<points.length-1;i++){
    const start=out[out.length-1],control=points[i],last=i===points.length-2,end=last?points[i+1]:[(points[i][0]+points[i+1][0])/2,(points[i][1]+points[i+1][1])/2];
    for(let s=1;s<=per;s++){const t=s/per,u=1-t;out.push([u*u*start[0]+2*u*t*control[0]+t*t*end[0],u*u*start[1]+2*u*t*control[1]+t*t*end[1]]);}
  }
  return out.map(([x,y])=>[round(x),round(y)]);
}

/**
 * Lay out the Antiquity bonsai. `collection` is the bonsai's collection, `chronology` its chronology (for the order
 * of lives) and `materialOf` maps a collection id to the id of its material; a branch's own star is
 * `antiquity:route:<branch id>`. Returns the place and role of every star, the links the tree can draw, the lines of
 * its ground, trunk, roots and limbs, its names, and its extent.
 */
export function antiquityTree({collection,chronology=null,materialOf}){
  const positions=new Map(),roles=new Map(),sides=new Map(),edges=[],lines=[],names=[];
  const set=(id,role,x,y,side=null)=>{if(!id||positions.has(id))return false;positions.set(id,[round(x),round(y),0]);roles.set(id,role);if(side)sides.set(id,side);return true;};
  const of=legacy=>legacy?materialOf(legacy)??null:null;
  const at=id=>positions.get(id);
  const entities=collection?.entities??[],kind=k=>entities.filter(item=>item.kind===k);
  const authors=kind('author'),works=kind('work'),versions=entities.filter(item=>['translation','original','edition'].includes(item.kind));
  const passages=collection?.passages??[],branches=collection?.branches??[],questions=collection?.questions??[],terms=collection?.terms??[],eras=collection?.eras??[];

  // ── The crown first: a passage a branch reads grows up into that branch, the first branch that reads it.
  const readBy=new Map();for(const branch of branches)for(const node of branch.nodes??[])if(!readBy.has(node.id))readBy.set(node.id,branch.id);

  // ── Roots. Authors in the order of their lives; under each, its works, the longest in the middle.
  const lifeStart=id=>{const situation=chronology?.situations?.[chronology?.lives?.[id]],year=parseInt(situation?.dating?.interval?.start,10);return Number.isFinite(year)?year:null;};
  const ordered=[...authors].sort((a,b)=>(lifeStart(a.id)??ERA_START[a.period]??0)-(lifeStart(b.id)??ERA_START[b.period]??0));
  const rooted=work=>passages.filter(p=>p.workId===work.id&&!readBy.has(p.id));
  const strands=new Map();let x=0,previous=null;
  const slots=new Map();
  for(const author of ordered){
    if(previous&&author.period!==previous)x+=ERA_GAP;previous=author.period;
    const own=works.filter(w=>w.author===author.id).sort((a,b)=>rooted(b).length-rooted(a).length),line=[];
    own.forEach((w,i)=>{if(i%2)line.push(w);else line.unshift(w);});
    // A work's other witnesses — its original, its translations and their editions — hang beside it.
    const kin=versions.filter(v=>own.some(w=>w.id===v.work)).sort((a,b)=>['original','translation','edition'].indexOf(a.kind)-['original','translation','edition'].indexOf(b.kind));
    const all=[...line,...kin],width=Math.max(EMPTY,all.length*STRAND);
    slots.set(author.id,{from:x,to:x+width,center:x+width/2,era:author.period,items:all});
    x+=width+GAP;
  }
  const shift=(x-GAP)/2;
  for(const [authorId,slot] of slots){
    slot.from-=shift;slot.to-=shift;slot.center-=shift;
    set(of(authorId),'author',slot.center,0,'above');
    const left=slot.from+(slot.to-slot.from-slot.items.length*STRAND)/2+STRAND/2;
    slot.items.forEach((w,i)=>{
      const wx=left+i*STRAND,version=w.kind!=='work',id=of(w.id);
      set(id,version?'version':'work',wx,62,'above');
      const hang=version?[]:rooted(w),lean=(wx-slot.center)*.05,beads=[];
      hang.forEach((p,j)=>{const px=wx+lean*(j+1)/Math.max(1,hang.length)*2,py=96+j*BEAD;if(set(of(p.id),'passage',px,py,'right'))beads.push([px,py]);});
      const bottom=beads.length?beads[beads.length-1][1]+14:version?78:86;
      lines.push({tone:'root',points:[...smooth([[slot.center,0],[slot.center,34],[wx,28],[wx,62]],5),...(beads.length?beads:[[wx,bottom]])]});
      strands.set(w.id,{x:wx,bottom});
    });
  }
  const eraSpan={};for(const slot of slots.values()){const span=eraSpan[slot.era]??=[Infinity,-Infinity];span[0]=Math.min(span[0],slot.from);span[1]=Math.max(span[1],slot.to);}
  const rootBottom=Math.max(140,...[...positions.values()].map(p=>p[1]))+40;
  // Translators stand under the translations they made.
  for(const translator of kind('translator')){const made=versions.find(v=>v.translator===translator.id),strand=made&&strands.get(made.id);if(strand)set(of(translator.id),'translator',strand.x,strand.bottom+40,'right');}
  // The ground, and under the roots the eras with their stars.
  const xs=[...positions.values()].map(p=>p[0]),groundFrom=Math.min(...xs)-40,groundTo=Math.max(...xs)+40;
  lines.push({tone:'ground',points:[[groundFrom,0],[groundTo,0]]});
  for(const era of eras){const span=eraSpan[era.period];if(!span)continue;lines.push({tone:'era',color:ERA_COLORS[era.period],points:[[span[0],rootBottom],[span[1],rootBottom]]});set(of(era.id),'era',(span[0]+span[1])/2,rootBottom,'below');}
  // Schools stand just above the ground, in their era.
  for(const era of Object.keys(eraSpan)){const own=kind('school').filter(s=>(SCHOOL_ERA[s.id]??'hellenistic')===era),span=eraSpan[era];own.forEach((s,i)=>set(of(s.id),'school',span[0]+(span[1]-span[0])*(i+1)/(own.length+1),-34,'above'));}

  // ── Trunk. Words low and concepts high, each on the side where its texts lie.
  const meanX=list=>{const values=list.map(id=>at(id)?.[0]).filter(Number.isFinite);return values.length?values.reduce((s,v)=>s+v,0)/values.length:0;};
  const termX=term=>meanX([...new Set((term.uses??[]).map(use=>use.work))].map(of));
  const sortedTerms=[...terms].sort((a,b)=>termX(a)-termX(b)),COLS=4;
  for(let c=0;c<COLS;c++){const column=sortedTerms.filter((_,i)=>Math.min(COLS-1,Math.floor(i*COLS/sortedTerms.length))===c);column.forEach((term,j)=>set(of(`lexical:${term.id}`),'term',-96+c*64+(j%2?8:-8),-92-j*25,'right'));}
  const conceptsOf=id=>passages.filter(p=>(p.concepts??[]).includes(id)).map(p=>of(p.id));
  const sortedConcepts=[...kind('concept')].sort((a,b)=>meanX(conceptsOf(a.id))-meanX(conceptsOf(b.id))),half=Math.ceil(sortedConcepts.length/2);
  [sortedConcepts.slice(0,half),sortedConcepts.slice(half)].forEach((column,side)=>{
    column.sort((a,b)=>conceptsOf(a.id).length-conceptsOf(b.id).length);
    column.forEach((concept,j)=>set(of(concept.id),'concept',(side?1:-1)*58,-430-j*54-(side?27:0),side?'right':'left'));
  });
  const trunkSide=m=>smooth([[m*178,0],[m*122,-260],[m*118,-620],[m*150,-T+30],[m*205,-T-60],[m*240,-T-110]],7);
  lines.push({tone:'trunk',points:trunkSide(-1)},{tone:'trunk',points:trunkSide(1)});

  // ── Crown. Seven areas fan out from the top of the trunk; a branch's moves stand depth by depth from its first.
  const areaIds=[...new Set(branches.map(b=>b.area))];
  const grown=branches.map(branch=>{
    const neighbours=new Map();for(const edge of branch.edges??[]){for(const [a,b] of [[edge.from,edge.to],[edge.to,edge.from]]){if(!neighbours.has(a))neighbours.set(a,[]);neighbours.get(a).push(b);}}
    const depth=new Map([[branch.root,0]]),queue=[branch.root];
    while(queue.length){const id=queue.shift();for(const next of neighbours.get(id)??[])if(!depth.has(next)){depth.set(next,depth.get(id)+1);queue.push(next);}}
    const levels=[];for(const node of branch.nodes??[]){const d=depth.get(node.id)??1;(levels[d]??=[]).push(node.id);}
    return {branch,levels:Array.from(levels,level=>level??[]),weight:Math.max(2.2,...Array.from(levels,level=>level?.length??0))};
  });
  const total=grown.reduce((s,g)=>s+g.weight,0),span=A1-A0-AREA_GAP*Math.max(0,areaIds.length-1);
  let angle=A0;const areaPlaces=[];
  areaIds.forEach((areaId,index)=>{
    const color=AREA_COLORS[index%AREA_COLORS.length],own=grown.filter(g=>g.branch.area===areaId),from=angle;
    const limbs=[];
    for(const g of own){
      const width=span*g.weight/total,a0=angle,a1=angle+width,mid=angle+width/2;angle=a1;
      g.levels.forEach((level,d)=>{const r=R0+d*RS;level.forEach((id,i)=>{const t=level.length===1?.5:i/(level.length-1),a=level.length===1?mid:a0+(a1-a0)*(.14+.72*t);const [px,py]=polar(r,a);if(readBy.get(id)===g.branch.id)set(of(id),'step',px,py,Math.cos(a)<0?'left':'right');});});
      const tip=polar(R0+(g.levels.length-1)*RS+92,mid);
      set(`antiquity:route:${g.branch.id}`,'branch',tip[0],tip[1],Math.sin(mid)<-.5?'above':Math.cos(mid)<0?'left':'right');
      g.mid=mid;g.color=color;limbs.push(g);
    }
    const middle=(from+angle)/2,joint=polar(R0-215,middle);
    areaPlaces.push({id:areaId,title:AREA_TITLES[areaId]??bi(areaId,areaId),branches:own.length,center:polar(R0+4*RS+520,middle),color});
    for(const g of limbs){
      const base=polar(R0-118,g.mid),first=at(of(g.branch.root))??polar(R0,g.mid);
      lines.push({tone:'limb',color,points:smooth([[0,-T+20],[joint[0]*.55,(joint[1]-T)/2-30],joint],6)},
        {tone:'bough',color,points:smooth([joint,[(joint[0]+base[0])/2,(joint[1]+base[1])/2-20],base,[first[0],first[1]]],6)});
    }
    angle+=AREA_GAP;
  });

  // ── The rim. Each question stands by the branches that ask it, else beside its kin by a shared concept.
  const askers=new Map();for(const g of grown)for(const q of g.branch.questions??[]){const id=typeof q==='string'?q:q.ru;if(!askers.has(id))askers.set(id,[]);askers.get(id).push(g.mid);}
  const want=new Map();for(const q of questions){const list=askers.get(q.id);if(list?.length)want.set(q.id,list.reduce((s,v)=>s+v,0)/list.length);}
  for(const q of questions)if(!want.has(q.id)){const mine=new Set(q.concepts??[]),kin=questions.filter(o=>want.has(o.id)&&(o.concepts??[]).some(c=>mine.has(c)));want.set(q.id,kin.length?kin.reduce((s,o)=>s+want.get(o.id),0)/kin.length:(A0+A1)/2);}
  const rim=[...questions].sort((a,b)=>want.get(a.id)-want.get(b.id)||a.id.localeCompare(b.id)),gap=(A1-A0)/(rim.length+1)*.92;
  const angles=rim.map(q=>want.get(q.id));
  for(let i=1;i<angles.length;i++)angles[i]=Math.max(angles[i],angles[i-1]+gap);
  const over=(angles.at(-1)??0)-(A1-.01);if(over>0)angles.forEach((_,i)=>{angles[i]-=over*(i/Math.max(1,angles.length-1));});
  for(let i=angles.length-2;i>=0;i--)angles[i]=Math.min(angles[i],angles[i+1]-gap);
  const RQ=R0+4*RS+175,RQ2=RQ+92;
  rim.forEach((q,i)=>{const a=angles[i],[px,py]=polar(i%2?RQ2:RQ,a);set(of(q.id),'question',px,py,Math.sin(a)<-.35?'above':Math.cos(a)<0?'left':'right');});

  // ── Links. Every one the collection states, drawn from star to star; between strata through the trunk.
  const regionOf=([px,py])=>py>-60?'roots':Math.abs(px)<190&&py>-T+40?'trunk':'crown';
  const via=(a,b)=>{
    const ra=regionOf(a),rb=regionOf(b);
    if(ra===rb){if(ra!=='roots')return [];return smooth([a,[(a[0]+b[0])/2,Math.min(a[1],b[1])-30-Math.abs(a[0]-b[0])*.08],b],5).slice(1,-1);}
    const low=ra==='roots'?a:rb==='roots'?b:null,high=low===a?b:a;
    let path;
    if(low&&regionOf(high)==='crown')path=[low,[low[0]*.3,-50],[0,-T*.5],[0,-T-40],high];
    else if(low)path=[low,[low[0]*.32,-46],high];
    else{const trunk=ra==='trunk'?a:b,crown=trunk===a?b:a;path=[trunk,[trunk[0]*.5,-T-30],crown];}
    if(path[0]!==a)path.reverse();
    return smooth(path,6).slice(1,-1);
  };
  const seen=new Set();
  const link=(from,to,edgeKind,label,extra={})=>{
    if(!from||!to||from===to||!positions.has(from)||!positions.has(to))return;
    const id=`tree:${edgeKind}:${from}>${to}`;if(seen.has(id))return;seen.add(id);
    edges.push({id,from,to,kind:edgeKind,label,via:via(at(from),at(to)).map(([px,py])=>[px,py,0]),...extra});
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

  // ── Names: the areas of the crown and the strata of the tree.
  const ys=[...positions.values()].map(p=>p[1]),bounds={x:[Math.min(groundFrom,...areaPlaces.map(a=>a.center[0]-150))-40,Math.max(groundTo,...areaPlaces.map(a=>a.center[0]+150))+40],y:[Math.min(...ys,...areaPlaces.map(a=>a.center[1]))-70,rootBottom+70]};
  const band=role=>{const list=[...roles].filter(([,r])=>r===role).map(([id])=>at(id)[1]);return list.length?[Math.min(...list),Math.max(...list)]:null;};
  const strata={crown:[Math.min(...['question','branch','step'].map(r=>band(r)?.[0]??0)),Math.max(...['question','branch','step'].map(r=>band(r)?.[1]??-T))],concepts:band('concept'),words:band('term'),texts:[-34,rootBottom]};
  for(const area of areaPlaces)names.push({id:`tree:area:${area.id}`,kind:'area',title:area.title,count:area.branches,center:[round(area.center[0]),round(area.center[1]),0],color:area.color});
  for(const [id,title] of TREE_STRATA){const range=strata[id];if(range)names.push({id:`tree:stratum:${id}`,kind:'stratum',title,center:[round(bounds.x[0]+30),round((range[0]+range[1])/2),0],range});}

  return {positions,roles,sides,edges,lines,names,bounds,regionOf:id=>at(id)?regionOf(at(id)):null};
}
