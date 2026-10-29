import {describe,expect,it} from 'vitest';
import collection from './antiquity-collection-data.json' with {type:'json'};
import {RESEARCH_ROUTES} from '../../constructor/atlas-routes.mjs';
import {PRIMARY_SOURCES} from '../../constructor/source-references.mjs';
import {INQUIRY_SOURCES_CONCEPTS} from '../../constructor/inquiry-sources-concepts.mjs';
import {INQUIRY_READINGS_ECHOES} from '../../constructor/inquiry-readings-echoes.mjs';
import {antiquityTimeline,bookPlace,dodgeRow,parseLifeYears,romanValue,stepTimeline,zarathustraTimeline} from './space-timeline.mjs';

describe('authored life dates',()=>{
  it('reads every form the collection uses, keeping how precise the line is',()=>{
    expect(parseLifeYears('469–399 BCE')).toEqual({start:-469,end:-399,precision:'year'});
    expect(parseLifeYears('c. 429–347 BCE')).toEqual({start:-429,end:-347,precision:'approx'});
    expect(parseLifeYears('45–47–c. 120 CE')).toEqual({start:45,end:120,precision:'approx'});
    expect(parseLifeYears('c. 1 BCE–65 CE')).toEqual({start:-1,end:65,precision:'approx'});
    expect(parseLifeYears('mid-90s to mid-50s BCE')).toEqual({start:-95,end:-55,precision:'decade'});
    expect(parseLifeYears('50s–c. 135 CE')).toEqual({start:55,end:135,precision:'decade'});
    expect(parseLifeYears('5th–4th c. BCE · early treatises of the corpus')).toEqual({start:-500,end:-301,precision:'century'});
    expect(parseLifeYears('3rd century CE · period of activity')).toEqual({start:201,end:300,precision:'century'});
    expect(parseLifeYears('')).toBeNull();
  });
  it('dates every author of the collection, inside the period the collection gives',()=>{
    const bounds={classical:[-500,-300],hellenistic:[-350,0],roman:[-100,300],late:[200,400]};
    for(const author of collection.entities.filter(entry=>entry.kind==='author')){
      const years=parseLifeYears(author.meta.en);expect(years,author.id).toBeTruthy();
      expect(years.start,author.id).toBeLessThanOrEqual(years.end);
      const middle=(years.start+years.end)/2,[low,high]=bounds[author.period];
      expect(middle,author.id).toBeGreaterThanOrEqual(low);expect(middle,author.id).toBeLessThanOrEqual(high);
    }
  });
});

describe('Antiquity on a scale of years',()=>{
  const entries=[...collection.entities,...collection.eras,...collection.passages];
  const routes=collection.branches.map(branch=>({id:branch.id,title:branch.rootTitle,steps:branch.nodes.map(node=>({nodeId:`antiquity:reading:${node.id}`}))}));
  const timeline=antiquityTimeline({routes,entries});
  it('places every move of every branch, one row per branch, one line per row',()=>{
    for(const branch of collection.branches)for(const node of branch.nodes)expect(timeline.positions.has(`antiquity:reading:${node.id}`),node.id).toBe(true);
    expect(timeline.lanes.map(lane=>lane.id)).toEqual(collection.branches.map(branch=>branch.id));
    const rows=new Set([...timeline.positions.values()].map(([,y])=>y));
    expect(rows.size).toBe(collection.branches.length);
  });
  it('keeps a move within its author\'s life, which the data dates instead of the move',()=>{
    const authors=new Map(collection.entities.filter(entry=>entry.kind==='author').map(author=>[author.id,parseLifeYears(author.meta.en)]));
    for(const [id,[x]] of timeline.positions){
      const life=timeline.lifeOf(id),years=authors.get(life.authorId),nominal=timeline.xOf(id);
      expect(life.start,id).toBe(years.start);expect(life.end,id).toBe(years.end);
      expect(nominal,id).toBeGreaterThanOrEqual(timeline.xOfYear(years.start));expect(nominal,id).toBeLessThanOrEqual(timeline.xOfYear(years.end));
      // Neighbours on one row keep a gap, so a star may stand a little off its value, never far.
      expect(Math.abs(x-nominal),id).toBeLessThanOrEqual(24);
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
  it('never puts two stars on one point',()=>{
    const seen=new Set();
    for(const [id,[x,y]] of timeline.positions){const key=`${x}:${y.toFixed(2)}`;expect(seen.has(key),id).toBe(false);seen.add(key);}
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
  it('spans an era over the lives of its authors in the collection, and rules every century',()=>{
    const labels=timeline.labels('en'),era=id=>labels.find(label=>label.id===`time:${id}`);
    expect(era('era-classical').span).toEqual([timeline.xOfYear(-500),timeline.xOfYear(-301)]);
    expect(era('era-late').span).toEqual([timeline.xOfYear(204),timeline.xOfYear(305)]);
    // Overlapping eras take alternate rows of the strip.
    expect(era('era-classical').strip).not.toBe(era('era-hellenistic').strip);
    const rules=labels.filter(label=>label.variant==='rule');
    expect(rules.map(label=>label.center[0])).toEqual([-500,-400,-300,-200,-100,0,100,200,300,400].map(timeline.xOfYear));
    expect(rules.filter(label=>label.tone==='strong').map(label=>label.center[0])).toEqual([timeline.xOfYear(0)]);
    expect(timeline.bounds).toEqual([timeline.xOfYear(-500),timeline.xOfYear(400)]);
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
