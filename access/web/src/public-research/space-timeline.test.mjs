import {describe,expect,it} from 'vitest';
import collection from '../../../../bonsai/antiquity/collection.json' with {type:'json'};
import chronology from '../../../../bonsai/antiquity/chronology.json' with {type:'json'};
import {readChronology} from '../../../../bonsai/lib/chronology.mjs';
import {RESEARCH_ROUTES} from '../../constructor/atlas-routes.mjs';
import {PRIMARY_SOURCES} from '../../constructor/source-references.mjs';
import {INQUIRY_SOURCES_CONCEPTS} from '../../constructor/inquiry-sources-concepts.mjs';
import {INQUIRY_READINGS_ECHOES} from '../../constructor/inquiry-readings-echoes.mjs';
import zarathustraChronology from '../../../../bonsai/zarathustra/chronology.json' with {type:'json'};
import {antiquityTimeline,bookPlace,dodgeRow,formatYears,romanValue,stackColumns,stepTimeline,toRoman,trajectory,zarathustraTimeline} from './space-timeline.mjs';
import {datingKind} from './space-measure.mjs';

describe('the lives the chronology dates',()=>{
  const reader=readChronology(chronology);
  it('dates every author of the collection from a named source, inside the period the collection gives',()=>{
    const bounds={classical:[-500,-300],hellenistic:[-350,0],roman:[-100,300],late:[100,400]};
    for(const author of collection.entities.filter(entry=>entry.kind==='author')){
      const id=reader.lifeOf(author.id);
      // A corpus of many hands has no life; its treatises keep their own datings.
      if(author.id==='hippocrates'){expect(id).toBeNull();continue;}
      const span=reader.span(id);expect(span,author.id).toBeTruthy();
      expect(reader.sources(id).length,author.id).toBeGreaterThan(0);
      expect(reader.sources(id).every(source=>!/example\.org/.test(source.url)),author.id).toBe(true);
      const middle=(span.from+span.to)/2,[low,high]=bounds[author.period];
      expect(middle,author.id).toBeGreaterThanOrEqual(low);expect(middle,author.id).toBeLessThanOrEqual(high);
    }
  });
  it('keeps a floruit apart from a life: what a source dates only by activity is called so',()=>{
    expect(reader.lifeOf('sextus')).toBe('tos.biographical-phase.sextus-empiricus.activity');
    expect(reader.situation(reader.lifeOf('sextus')).dating.certainty).toBe('uncertain');
    expect(reader.lifeOf('laertius')).toBe('tos.biographical-phase.diogenes-laertius.activity');
  });
});

describe('Antiquity on a scale of years',()=>{
  const entries=[...collection.entities,...collection.eras,...collection.passages];
  const routes=collection.branches.map(branch=>({id:branch.id,title:branch.rootTitle,steps:branch.nodes.map(node=>({nodeId:`antiquity:reading:${node.id}`}))}));
  const timeline=antiquityTimeline({routes,entries,chronology}),reader=readChronology(chronology);
  it('places every move of every branch, one row per branch, one line per row',()=>{
    for(const branch of collection.branches)for(const node of branch.nodes)expect(timeline.positions.has(`antiquity:reading:${node.id}`),node.id).toBe(true);
    expect(timeline.lanes.map(lane=>lane.id)).toEqual(collection.branches.map(branch=>branch.id));
    const rows=new Set([...timeline.positions.values()].map(([,y])=>Math.round(y/36)));
    expect(rows.size).toBe(collection.branches.length);
  });
  it('needs a chronology: without one it measures nothing',()=>{
    expect(antiquityTimeline({routes,entries})).toBeNull();
  });
  it('keeps every move within the span its writing is dated to, which is never a guess',()=>{
    for(const [id,[x]] of timeline.positions){
      const span=timeline.spanOf(id),time=timeline.timeOf(id);
      expect(time.composition,id).toBeTruthy();expect(span,id).toBeTruthy();
      expect(x,id).toBeGreaterThanOrEqual(timeline.xOfYear(span.from)-1e-9);expect(x,id).toBeLessThanOrEqual(timeline.xOfYear(span.to)+1e-9);
      // Neighbours on one row keep a gap, so a star may stand a little off its value, never outside its span.
      expect(Math.abs(x-timeline.xOf(id)),id).toBeLessThanOrEqual(24);
    }
  });
  it('keeps time on the horizontal: a later author stands to the right',()=>{
    const x=id=>timeline.positions.get(`antiquity:reading:${id}`)[0];
    const byAuthor=author=>collection.passages.find(passage=>passage.author===author&&timeline.positions.has(`antiquity:reading:${passage.id}`))?.id;
    expect(x(byAuthor('plato'))).toBeLessThan(x(byAuthor('aristotle')));
    expect(x(byAuthor('aristotle'))).toBeLessThan(x(byAuthor('epicurus')));
    expect(x(byAuthor('epicurus'))).toBeLessThan(x(byAuthor('seneca')));
    expect(x(byAuthor('seneca'))).toBeLessThan(x(byAuthor('sextus')));
  });
  it('orders Plato\'s dialogues by their phases and puts Cicero\'s On Duties in 44 BCE',()=>{
    const x=passage=>timeline.positions.get(`antiquity:reading:${passage}`)[0];
    const of=work=>collection.passages.find(passage=>passage.workId===work&&timeline.positions.has(`antiquity:reading:${passage.id}`)).id;
    expect(x(of('apology'))).toBeLessThan(x(of('meno')));
    expect(x(of('meno'))).toBeLessThan(x(of('republic')));
    expect(x(of('republic'))).toBeLessThan(x(of('sophist')));
    expect(x(of('sophist'))).toBeLessThan(x(of('metaphysics')));
    const officium=x(of('de-officiis'));expect(officium).toBeGreaterThanOrEqual(timeline.xOfYear(-43));expect(officium).toBeLessThanOrEqual(timeline.xOfYear(-42));
  });
  it('never puts two stars on one point, and stars that share a dating stand in a short column',()=>{
    const seen=new Set();
    for(const [id,[x,y]] of timeline.positions){
      const key=`${x.toFixed(3)}:${y.toFixed(3)}`;expect(seen.has(key),id).toBe(false);seen.add(key);
      const lane=timeline.lanes.reduce((a,b)=>Math.abs(b.y-y)<Math.abs(a.y-y)?b:a);expect(Math.abs(y-lane.y),id).toBeLessThanOrEqual(21);
    }
    const column=stackColumns(new Map([['a',0],['b',1],['c',2],['d',40]]),6,7);
    expect([column.get('a'),column.get('b'),column.get('c'),column.get('d')]).toEqual([0,-7,7,0]);
  });
  it('labels centuries, eras and rows in both languages',()=>{
    const ru=timeline.labels('ru'),en=timeline.labels('en');
    expect(ru.filter(label=>label.variant==='axis').map(label=>label.title)).toEqual(['V в.','IV в.','III в.','II в.','I в.','I в.','II в.','III в.','IV в.']);
    expect(en.filter(label=>label.variant==='axis').map(label=>label.title)).toContain('4th c.');
    expect(ru.filter(label=>label.variant==='side').map(label=>label.title)).toEqual(['до н. э.','н. э.']);
    expect(ru.filter(label=>label.variant==='era').map(label=>label.title)).toEqual(['Классическая Греция','Эллинистический мир','Римский мир','Поздняя Античность']);
    const lanes=ru.filter(label=>label.variant==='lane');
    expect(lanes.map(label=>label.ref)).toEqual(collection.branches.map(branch=>branch.id));
    expect(ru.filter(label=>label.variant==='thread')).toHaveLength(collection.branches.length);
  });
  it('rules each century where it begins, in astronomical years, and spans an era over its authors\' lives',()=>{
    const labels=timeline.labels('en'),rules=labels.filter(label=>label.variant==='rule');
    // A century begins at its year 100k + 1: 400 BCE is the astronomical year −399, 1 CE is 1.
    expect(rules.map(label=>label.center[0])).toEqual([-499,-399,-299,-199,-99,1,101,201,301,401].map(timeline.xOfYear));
    expect(rules.filter(label=>label.tone==='strong').map(label=>label.center[0])).toEqual([timeline.xOfYear(1)]);
    expect(timeline.bounds).toEqual([timeline.xOfYear(-499),timeline.xOfYear(401)]);
    const era=id=>labels.find(label=>label.id===`time:${id}`);
    const lives=collection.entities.filter(entry=>entry.kind==='author'&&entry.period==='classical'&&reader.lifeOf(entry.id)).map(entry=>reader.span(reader.lifeOf(entry.id)));
    expect(era('era-classical').span).toEqual([timeline.xOfYear(Math.min(...lives.map(span=>span.from))),timeline.xOfYear(Math.max(...lives.map(span=>span.to)))]);
    // Overlapping eras take alternate rows of the strip.
    expect(era('era-classical').strip).not.toBe(era('era-hellenistic').strip);
  });
});

describe('the scale of years at its edges',()=>{
  // A small collection: one author with a life, a corpus without one, and writings dated in each way.
  const date=(kind,extra)=>({kind,role:'historical-time',calendar:null,year_numbering:kind==='relative-order'?null:'astronomical',certainty:'approximate',source_wording:{text:'x',language:'en'},...extra});
  const situation=(type,dating)=>({type,label:{ru:'x',en:'x'},dating,statement:{ru:'x',en:'x'},sources:['s'],footing:'high',epistemic:'reported'});
  const entries=[{id:'a',kind:'author',period:'classical'},{id:'h',kind:'author',period:'classical'},
    ...['w','c','u','b'].map(id=>({id,kind:'work',author:id==='c'?'h':'a'})),...['w','c','u','b'].map(id=>({id:`p-${id}`,kind:'passage',workId:id}))];
  const chronologyWith=extra=>({schema:'tos.bonsai.chronology.v1',sources:{s:{short:'S',title:'S',url:'https://example.org/'}},lives:{a:'tos.biographical-phase.a.lifetime'},
    compositions:{works:{w:'tos.historical-process.a.w-composition',c:'tos.historical-process.h.c-composition',u:'tos.historical-process.a.u-composition',b:'tos.historical-process.a.b-composition'}},
    situations:{'tos.biographical-phase.a.lifetime':situation('biographical-phase',date('interval-assertion',{interval:{start:'-0427',end:'-0346'}})),
      'tos.historical-process.a.w-composition':situation('historical-process',date('relative-order',{relative:{relation:'during',anchor_ref:'tos.biographical-phase.a.lifetime'}})),
      // A corpus of many hands dated before every life of the collection.
      'tos.historical-process.h.c-composition':situation('historical-process',date('interval-assertion',{interval:{start:'-0649',end:'-0600'}})),
      'tos.historical-process.a.u-composition':situation('historical-process',{...date('unknown-date'),certainty:'unknown'}),
      ...extra}});
  const routes=[{id:'r',title:{ru:'Ветвь',en:'Branch'},steps:['w','c','u','b'].map(id=>({nodeId:`antiquity:reading:p-${id}`}))}];
  it('spans every dated writing, also one outside every life, and names the column past it by what stands there',()=>{
    const known=antiquityTimeline({routes,entries,chronology:chronologyWith({'tos.historical-process.a.b-composition':situation('historical-process',{...date('unknown-date'),certainty:'unknown'})})});
    expect(known.domain.min).toBe(-699);
    const x=known.positions.get('antiquity:reading:p-c')[0];expect(x).toBeGreaterThanOrEqual(known.xOfYear(-649));expect(x).toBeLessThanOrEqual(known.xOfYear(-599));
    expect(known.labels('ru').find(label=>label.id==='time:undated').title).toBe('дата неизвестна');
    // A writing dated only "before" another stands past the scale too, but its date is not unknown.
    const before=antiquityTimeline({routes,entries,chronology:chronologyWith({'tos.historical-process.a.b-composition':situation('historical-process',date('relative-order',{relative:{relation:'before',anchor_ref:'tos.biographical-phase.a.lifetime'}}))})});
    expect(before.isOutside('antiquity:reading:p-b')).toBe(true);
    expect(before.labels('ru').find(label=>label.id==='time:undated').title).toBe('вне шкалы');
    expect(before.labels('en').find(label=>label.id==='time:undated').title).toBe('off the scale');
  });
  it('counts chapters in Roman numerals up to the last one the book has',()=>{
    expect([toRoman(1),toRoman(4),toRoman(46),toRoman(79),toRoman(80)]).toEqual(['I','IV','XLVI','LXXIX','LXXX']);
    for(const n of [1,9,14,40,49,80,99])expect(romanValue(toRoman(n)),String(n)).toBe(n);
  });
});

describe('the Antiquity chronology',()=>{
  const reader=readChronology(chronology),works=new Map(collection.entities.filter(entry=>entry.kind==='work').map(work=>[work.id,work]));
  const walked=new Set(collection.branches.flatMap(branch=>branch.nodes.map(node=>node.source.workId)));
  it('names its posture in both languages and every source it rests on',()=>{
    expect(chronology.posture.ru).toMatch(/не канон/);expect(chronology.posture.en).toMatch(/not canon/);
    expect(chronology.checked).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    for(const [id,source] of Object.entries(chronology.sources)){expect(source.url,id).toMatch(/^https:\/\//);expect(source.short,id).toBeTruthy();expect(source.title,id).toBeTruthy();}
  });
  it('dates the writing of every work a branch walks, each in a kind of the Tree\'s date grammar',()=>{
    const kinds=new Map();
    for(const workId of walked){
      const situation=reader.compositionOf({workId});expect(situation,workId).toBeTruthy();
      const kind=datingKind(situation,reader);kinds.set(kind,(kinds.get(kind)??0)+1);
      expect(reader.situation(situation).work,workId).toBe(workId);
      expect(reader.span(situation),workId).toBeTruthy();
    }
    expect(kinds.get('date-assertion')).toBeGreaterThanOrEqual(1);
    expect(kinds.get('interval-assertion')).toBeGreaterThan(5);
    expect(kinds.get('relative-phase')).toBeGreaterThan(20);
    // "Within the author's life" is a stated dating, not a fallback.
    for(const workId of ['menoeceus','doctrines','adversus-coloten'])expect(datingKind(reader.compositionOf({workId}),reader),workId).toBe('relative-life');
    expect(datingKind(reader.compositionOf({workId:'outlines'}),reader)).toBe('relative-activity');
  });
  it('keeps a writing within its author\'s dated life, and a weak footing marked weak',()=>{
    for(const workId of walked){
      const situation=reader.compositionOf({workId}),span=reader.span(situation),lifeId=reader.lifeOf(works.get(workId).author),life=lifeId?reader.span(lifeId):null;
      if(life){expect(span.from,workId).toBeGreaterThanOrEqual(life.from);expect(span.to,workId).toBeLessThanOrEqual(life.to);}
      const sources=reader.sources(situation);expect(sources.length,workId).toBeGreaterThan(0);
      if(sources.every(source=>/wikipedia\.org/.test(source.url)))expect(reader.situation(situation).footing,workId).not.toBe('high');
    }
  });
  it('prefers a passage\'s own writing to its work\'s and keeps each source with its dating',()=>{
    expect(reader.span(reader.compositionOf({passageId:'plotinus-sculpture',workId:'enneads'}))).toMatchObject({first:253,last:263});
    expect(reader.span(reader.compositionOf({workId:'enneads'}))).toMatchObject({first:253,last:270});
    expect(reader.sources(reader.compositionOf({workId:'laws'})).map(source=>source.id)).toEqual(['iep-plato','britannica-plato-late']);
    expect(reader.span(reader.compositionOf({workId:'apology'}))).toMatchObject({first:-398,last:-386,relation:'during',anchor:'tos.biographical-phase.plato.early-dialogues'});
    expect(reader.situation(reader.compositionOf({workId:'memorabilia'})).footing).toBe('low');
    expect(reader.situation(reader.compositionOf({workId:'rhetoric'})).footing).toBe('high');
    expect(reader.situation(reader.compositionOf({workId:'sacred-disease'})).display).toEqual({ru:'вторая половина V в. до н. э.',en:'second half of the 5th c. BCE'});
    expect(formatYears(reader.span(reader.compositionOf({workId:'de-officiis'})),'ru').replaceAll('\u00a0',' ')).toBe('44 до н. э.');
  });
});

describe('a concept traced on a scale',()=>{
  it('joins the stars that carry it in the order of the scale, leaving out what the scale cannot place',()=>{
    const xs={a:30,b:10,c:20,d:NaN};
    expect(trajectory(['a','b','c','d','a'],id=>xs[id])).toEqual({ids:['b','c','a'],links:[['b','c'],['c','a']]});
  });
});

describe('Zarathustra on the order of the book',()=>{
  it('reads the part and chapter a reference points to',()=>{
    expect(romanValue('XLVI')).toBe(46);expect(romanValue('LXXIX')).toBe(79);expect(romanValue('')).toBeNaN();
    expect(bookPlace(PRIMARY_SOURCES['z-vision'])).toEqual({part:3,chapter:46});
    expect(bookPlace(PRIMARY_SOURCES['z-prologue'])).toEqual({part:1,chapter:0});
    expect(bookPlace(PRIMARY_SOURCES['z-metamorphoses'])).toEqual({part:1,chapter:1});
    expect(bookPlace(PRIMARY_SOURCES['z-sign'])).toEqual({part:4,chapter:80});
    expect(bookPlace(PRIMARY_SOURCES['gs341'])).toBeNull();
    for(const [id,reference] of Object.entries(PRIMARY_SOURCES))if(id.startsWith('z-'))expect(bookPlace(reference),id).toBeTruthy();
  });
  it('places each grounded star at the first chapter it rests on, and the rest outside the scale',()=>{
    const inquiry={...INQUIRY_SOURCES_CONCEPTS,...INQUIRY_READINGS_ECHOES};
    const materials=Object.entries(inquiry).map(([id,value],index)=>({id,cluster:['sources','images','concepts'][index%3],inquiry:value}));
    const timeline=zarathustraTimeline({materials,references:PRIMARY_SOURCES,clusters:[{id:'sources',title:{ru:'Источники'}},{id:'images',title:{ru:'Образы'}},{id:'concepts',title:{ru:'Понятия'}}]});
    expect(timeline.positions.size).toBe(materials.length);
    expect(timeline.placeOf('demo:fate')).toEqual({part:2,chapter:24});
    expect(timeline.isOutside('demo:spinoza')).toBe(true);
    expect(timeline.xOf('demo:spinoza')).toBeGreaterThan(timeline.xOf('demo:fate'));
    const labels=timeline.labels('ru');
    expect(labels.some(label=>label.title==='вне книги'&&label.variant==='side')).toBe(true);
    expect(labels.filter(label=>label.variant==='era').map(label=>label.title)).toEqual(['Часть I','Часть II','Часть III','Часть IV']);
    // With its bonsai, each part names the years it was written.
    const dated=zarathustraTimeline({materials,references:PRIMARY_SOURCES,clusters:[{id:'sources',title:{ru:'Источники'}}],chronology:zarathustraChronology});
    expect(dated.labels('en').filter(label=>label.variant==='era').map(label=>label.title)).toEqual(['Part I · 1883','Part II · 1883','Part III · 1884','Part IV · 1884–1885']);
    // A part spans the chapters the map cites from it; the texts outside the book stand past a strong rule.
    const part=labels.find(label=>label.id==='time:part2');expect(part.span[0]).toBeLessThanOrEqual(timeline.xOf('demo:fate'));expect(part.span[1]).toBeGreaterThanOrEqual(timeline.xOf('demo:fate'));
    const outside=labels.find(label=>label.id==='time:rule:outside');expect(outside.tone).toBe('strong');
    expect(outside.center[0]).toBeLessThan(timeline.xOf('demo:spinoza'));expect(outside.center[0]).toBeGreaterThan(Math.max(...[...timeline.positions.entries()].filter(([id])=>!timeline.isOutside(id)).map(([,[x]])=>x)));
  });
});

describe('one row on one line',()=>{
  it('keeps the order, a minimal gap and each star as near its value as the gap allows',()=>{
    const items=[{id:'a',x:0},{id:'b',x:2},{id:'c',x:4},{id:'d',x:100},{id:'e',x:4,order:5}];
    const xs=dodgeRow(items,12),order=[...xs.entries()].sort((a,b)=>a[1]-b[1]).map(([id])=>id);
    expect(order).toEqual(['a','b','c','e','d']);
    const sorted=[...xs.values()].sort((a,b)=>a-b);for(let i=1;i<sorted.length;i++)expect(sorted[i]-sorted[i-1]).toBeGreaterThanOrEqual(12-1e-9);
    // A crowded group is centred on its values; a lone star keeps its own.
    expect(xs.get('d')).toBe(100);
    expect((xs.get('a')+xs.get('b')+xs.get('c')+xs.get('e'))/4).toBeCloseTo((0+2+4+4)/4,9);
  });
});

describe('an open route on the same scale',()=>{
  it('keeps each step at its time and reads the steps top to bottom',()=>{
    const route=RESEARCH_ROUTES[0],ids=route.steps.map(step=>step.nodeId),xOf=id=>ids.indexOf(id)*10;
    const positions=stepTimeline(ids,xOf);
    expect([...positions.keys()]).toEqual(ids);
    const ys=ids.map(id=>positions.get(id)[1]);expect([...ys].sort((a,b)=>a-b)).toEqual(ys);
    expect(positions.get(ids[2])[0]).toBe(20);
  });
});
