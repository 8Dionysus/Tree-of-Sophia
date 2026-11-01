import {join} from 'node:path';
import {describe,expect,it} from 'vitest';
import {coreIssues,loadCore} from '../../../../bonsai/lib/core.mjs';
import {loadBonsai,loadWitnessChapters} from '../../../../bonsai/lib/graft.mjs';
import {semanticRegistry} from '../../../../bonsai/lib/objects.mjs';

const root=new URL('../../../../',import.meta.url).pathname;
const {home,manifest}=loadBonsai(root,'zarathustra');
const chapters=loadWitnessChapters(home,manifest),registry=semanticRegistry(root);
const fresh=()=>loadCore(join(home,'core'));
const concept=(core,id)=>core.concepts.find(node=>node.id===id);

describe('the philosophical core of the Zarathustra bonsai',()=>{
  it('holds every place, claim, relation and control case',()=>{
    expect(coreIssues(fresh(),chapters,registry)).toEqual([]);
  });

  it('keeps every German unit in one primary voice and every formula in the voice that says it',()=>{
    const core=fresh();
    expect(Object.keys(core.voices).sort()).toEqual(Object.keys(chapters).sort());
    for(const node of [...core.concepts.map(item=>item.conception),...core.figures.map(item=>item.essence)])expect(node.formula.voice.speaker).toBeTruthy();
  });

  it('keeps German figure names and only word forms in the three witness lexicons',()=>{
    const core=fresh();
    for(const figure of core.figures)expect(figure.names.de.some(item=>typeof item.form==='string')).toBe(true);
    for(const node of core.concepts){
      expect(Object.keys(node.lexicon).sort()).toEqual(['de','en','ru']);
      for(const forms of Object.values(node.lexicon))expect(forms.every(item=>typeof item.form==='string'&&Number.isFinite(item.count))).toBe(true);
    }
  });

  it('admits a claim only as its verdict allows and keeps the maker’s wording beside a revision',()=>{
    const core=fresh();
    for(const link of core.links){
      if(link.assessment.verdict==='revise'&&link.proposed?.verb)expect(link.verb.en).toBeTruthy();
      expect({map:['confirmed','reconstruction','revise'],held:['undecided'],rejected:['rejected']}[link.admission]).toContain(link.assessment.verdict);
    }
  });

  it('fails a formula moved into the animals’ song',()=>{
    const core=fresh();
    concept(core,'wheel').conception.formula.cite={w:'also-sprach-zarathustra',c:'III.13',u:['tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39','tos.text-unit.sid-c7a842d1445462c0d0df8eafd83efd39'],q:'ewig rollt das Rad des Seins'};
    expect(coreIssues(core,chapters,registry)).toContain('control animals-wheel: wheel has its formula at wheel');
  });

  it('fails a quotation the place does not hold and an admission its verdict refuses',()=>{
    const core=fresh();
    core.links[0].grounds[0]={...core.links[0].grounds[0],q:'nicht im Text'};
    core.links[1].assessment={...core.links[1].assessment,verdict:'rejected'};
    const problems=coreIssues(core,chapters,registry);
    expect(problems.some(issue=>issue.includes('"nicht im Text" is not at'))).toBe(true);
    expect(problems.some(issue=>issue.includes('admitted as map with verdict rejected'))).toBe(true);
  });

  it('fails when the dwarf loses his position against the recurrence',()=>{
    const core=fresh();
    const recurrence=concept(core,'recurrence');
    recurrence.counter_voices=recurrence.counter_voices.filter(voice=>voice.speaker!=='dwarf');
    const dwarf=core.figures.find(node=>node.id==='dwarf');
    dwarf.essence.counter_to=(dwarf.essence.counter_to??[]).filter(item=>item.concept!=='recurrence');
    expect(coreIssues(core,chapters,registry)).toContain('control dwarf-circle: no position of dwarf against recurrence');
  });
});
