import {describe,expect,it} from 'vitest';
import collection from './antiquity-collection-data.json' with {type:'json'};
import {ANTIQUITY_TRANSITIONS,SHARED_RELATION_LINES,isAuthoredTransition,partName,referenceLine,relationStyle,searchScore,sourceLine,stepWitness} from './space-legibility.mjs';
import {RELATIONS} from '../../constructor/atlas-view.mjs';
import {RESEARCH_ROUTES} from '../../constructor/atlas-routes.mjs';
import {PRIMARY_SOURCES} from '../../constructor/source-references.mjs';

describe('Antiquity link vocabulary',()=>{
  it('restores the four authored kinds, told apart by word, line and weight rather than hue alone',()=>{
    const kinds=new Set(collection.branches.flatMap(branch=>branch.edges.map(edge=>edge.kind)));
    expect([...kinds].sort()).toEqual(Object.keys(ANTIQUITY_TRANSITIONS).sort());
    const styles=Object.values(ANTIQUITY_TRANSITIONS);
    expect(new Set(styles.map(style=>style.ru)).size).toBe(4);
    expect(new Set(styles.map(style=>style.en)).size).toBe(4);
    expect(new Set(styles.map(style=>`${style.dash}:${style.weight}`)).size).toBe(4);
    for(const kind of Object.keys(ANTIQUITY_TRANSITIONS))expect(relationStyle('antiquity',kind,RELATIONS[kind])).toMatchObject({ru:ANTIQUITY_TRANSITIONS[kind].ru,dash:ANTIQUITY_TRANSITIONS[kind].dash});
  });
  it('gives every shared kind its own word and a line that differs beyond hue',()=>{
    const kinds=Object.keys(RELATIONS);
    expect(kinds.filter(kind=>!SHARED_RELATION_LINES[kind])).toEqual([]);
    const styles=kinds.map(kind=>relationStyle('zarathustra',kind,RELATIONS[kind]));
    expect(new Set(styles.map(style=>style.ru)).size).toBe(kinds.length);
    expect(new Set(styles.map(style=>`${style.dash}:${style.weight}`)).size).toBeGreaterThanOrEqual(kinds.length-2);
    expect(relationStyle('zarathustra','questions',RELATIONS.questions)).toMatchObject({ru:RELATIONS.questions.ru,dash:'dashed'});
    expect(relationStyle('zarathustra','contrasts',RELATIONS.contrasts)).toMatchObject({ru:RELATIONS.contrasts.ru,dash:'dashed'});
    expect(relationStyle('zarathustra','interprets',RELATIONS.interprets).dash).toBe('dotted');
    expect(relationStyle('zarathustra','supports',RELATIONS.supports)).toMatchObject({dash:'solid',hint:{ru:'служит основанием'}});
    expect(relationStyle('antiquity','supports',RELATIONS.supports)).toMatchObject({ru:RELATIONS.supports.ru});
    // Kinds of the same line family still differ by weight.
    for(const dash of ['solid','dotted','dashed']){const family=styles.filter(style=>style.dash===dash&&style.en!=='Contains'&&style.en!=='Translates');expect(new Set(family.map(style=>style.weight)).size,dash).toBe(family.length);}
  });
  it('recognizes only authored step-to-step transitions',()=>{
    expect(isAuthoredTransition({id:'antiquity:relation:branch-knowledge:apology-21d:meno-search'})).toBe(true);
    expect(isAuthoredTransition({id:'atlas:antiquity:relation:branch-knowledge:apology-21d:meno-search'})).toBe(true);
    expect(isAuthoredTransition({id:'antiquity:route-entry:branch-knowledge'})).toBe(false);
    expect(isAuthoredTransition({id:'antiquity:reading-source:apology-21d'})).toBe(false);
  });
});

describe('Antiquity names and witnesses',()=>{
  const items=[...collection.entities,...collection.passages];
  const byLegacy=new Map(items.map(item=>[item.id,item]));
  it('calls a route a branch and a passage a move only where a branch steps through it',()=>{
    const moveIds=new Set(['antiquity:reading:apology-21d']);
    expect(partName('antiquity',{id:'antiquity:route:branch-knowledge',route:{id:'branch-knowledge'}},'ru',{moveIds})).toBe('Ветвь');
    expect(partName('antiquity',{id:'antiquity:reading:apology-21d',kind:'interpretation'},'en',{moveIds})).toBe('Move');
    expect(partName('antiquity',{id:'antiquity:reading:apology-judge',kind:'fragment'},'ru',{moveIds})).toBe('');
    expect(partName('zarathustra',{id:'antiquity:route:x',route:{}},'ru',{moveIds})).toBe('');
  });
  it('names every move of every branch by author, work and place',()=>{
    for(const branch of collection.branches)for(const node of branch.nodes){
      const passage=byLegacy.get(node.id);expect(passage,node.id).toBeTruthy();
      const material={id:`antiquity:reading:${node.id}`,kind:'fragment',workId:passage.workId,author:passage.author,meta:passage.meta};
      for(const locale of ['ru','en']){
        const line=sourceLine(material,{materials:new Map(),entryByLegacyId:byLegacy,locale});
        const work=byLegacy.get(passage.workId),author=byLegacy.get(passage.author??work?.author);
        expect(line,node.id).toContain(work.title[locale]);expect(line,node.id).toContain(passage.meta[locale]);
        if(author)expect(line,node.id).toContain(author.title[locale]);
      }
    }
  });
  it('names the authored work of a passage read inside a containing book',()=>{
    const materials=new Map([['antiquity:source:menoeceus-death',{workId:'lives',locator:'X.124–125'}]]);
    expect(sourceLine({id:'antiquity:reading:menoeceus-death',kind:'interpretation',sourceNodeId:'antiquity:source:menoeceus-death'},{materials,entryByLegacyId:byLegacy,locale:'ru'})).toBe('Эпикур · Письмо к Менекею · §§124–125');
  });
  it('uses the cited source of a space move when the move itself has no work',()=>{
    const materials=new Map([['antiquity:source:meno-aporia',{workId:'meno',locator:'84a–b'}]]);
    expect(sourceLine({id:'antiquity:reading:unlisted',kind:'interpretation',sourceNodeId:'antiquity:source:meno-aporia'},{materials,entryByLegacyId:byLegacy,locale:'ru'})).toBe('Платон · Менон · 84a–b');
  });
});

describe('search order',()=>{
  it('puts a branch called by its own name before materials that only mention it',()=>{
    const branch={title:{ru:'Душа и смерть'},route:{steps:[1,2,3]}},concept={title:{ru:'Душа'},body:{ru:'Душа и смерть в «Федоне»'}},passage={title:{ru:'Определить смерть как отделение'}};
    const score=item=>searchScore(item,'Душа и смерть',{locale:'ru'});
    expect(score(branch)).toBeGreaterThan(score(concept));expect(score(branch)).toBeGreaterThan(score(passage));
    expect(searchScore(branch,'ДУША И СМЕРТЬ',{locale:'ru'})).toBe(score(branch));
    expect(searchScore({title:{ru:'Ещё'}},'еще',{locale:'ru'})).toBeGreaterThan(0);
    expect(searchScore(passage,'',{locale:'ru'})).toBe(0);
  });
});

describe('Zarathustra steps and transitions',()=>{
  it('names the author, work and place of every step of every route from its own grounds',()=>{
    for(const route of RESEARCH_ROUTES)for(const step of route.steps)for(const locale of ['ru','en']){
      const lines=stepWitness(step,{references:PRIMARY_SOURCES,locale});
      expect(lines.length,`${route.id}:${step.nodeId}`).toBeGreaterThan(0);
      for(const [index,ground] of step.grounds.entries()){const source=PRIMARY_SOURCES[ground.ref];expect(lines.join(' / ')).toContain(source.work[locale]);if(index===0)expect(lines[0]).toContain(source.author[locale]);}
    }
  });
  it('keeps one line per distinct ground and nothing for an unknown reference',()=>{
    expect(stepWitness({grounds:[{ref:'z-happy-isles'},{ref:'z-happy-isles'},{ref:'camus-sisyphus'}]},{references:PRIMARY_SOURCES,locale:'ru'})).toEqual(['Фридрих Ницше · Так говорил Заратустра · II · На блаженных островах','Альбер Камю · Миф о Сизифе · Заключительный раздел «Миф о Сизифе»']);
    expect(stepWitness({grounds:[{ref:'missing'}]},{references:PRIMARY_SOURCES,locale:'ru'})).toEqual([]);
    expect(referenceLine(null,'ru')).toBe('');
  });
  it('can name every route transition by the kind of the atlas link it walks',()=>{
    for(const route of RESEARCH_ROUTES)for(const transition of route.transitions){
      const [prefix,from,kind,to]=String(transition.edgeId).split(':');
      expect(prefix).toBe('atlas');expect(RELATIONS[kind],transition.edgeId).toBeTruthy();
      // A route may walk a link either way; the dock states the link as authored.
      expect([`${from}>${to}`,`${to}>${from}`]).toContain(`${transition.from.replace(/^demo:/,'')}>${transition.to.replace(/^demo:/,'')}`);
    }
  });
});
