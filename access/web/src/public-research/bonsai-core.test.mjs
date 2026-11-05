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

  it('reads every link and chain step as a sentence of its ends: the verb is a predicate with no subject of its own',()=>{
    const core=fresh(),verbs=[...core.links.map(link=>[link.id,link.verb]),...core.chains.flatMap(chain=>(chain.steps??[]).map((step,index)=>[`${chain.id}:${index}`,step.verb]))];
    // A verb that opens with a figure, a crowd or a pronoun names its own subject, and the sentence “from — verb — to” loses its ends.
    const subject=/^(звери|карлик|толпа|народ|заратустра|тень|один из|его |её |их |им |чью )/i;
    expect(verbs.filter(([,verb])=>subject.test(verb?.ru??'')).map(([id,verb])=>`${id}: ${verb.ru}`)).toEqual([]);
    // A sentence with a participant the place names keeps it: the child shows the distorted teaching to Zarathustra.
    expect(core.links.find(link=>link.id==='child:link:06:child-mirror').verb.ru).toContain('Заратустре');
  });

  it('keeps every German unit in one primary voice and every formula in the voice that says it',()=>{
    const core=fresh();
    expect(Object.keys(core.voices).sort()).toEqual(Object.keys(chapters).sort());
    for(const node of [...core.concepts.map(item=>item.conception),...core.figures.map(item=>item.essence)])expect(node.formula.voice.speaker).toBeTruthy();
  });

  it('keeps every voice descriptor source-scoped without inventing a narrator person',()=>{
    const core=fresh(),segments=Object.values(core.voices).flat();
    expect(segments).toHaveLength(775);
    expect(new Set(segments.map(segment=>segment.voice_descriptor.id)).size).toBe(775);
    expect(segments.filter(segment=>segment.voice_descriptor.identity_status==='narrator-role-only')).toHaveLength(208);
    const narrator=segments.find(segment=>segment.voice_descriptor.identity_status==='narrator-role-only');
    expect(narrator.voice_descriptor.role).toBe('work-narrator');
    expect(narrator.voice_descriptor.figure_ref).toBeUndefined();
    narrator.voice_descriptor.speaker_designation='someone-else';
    expect(coreIssues(core,chapters,registry).some(issue=>issue.includes('descriptor disagrees with segment'))).toBe(true);
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

  it('keeps broad families separate from exact relation types across all links',()=>{
    const core=fresh();
    expect(core.links).toHaveLength(400);
    expect(core.links.every(link=>link.relation_family?.family&&link.relation_family?.family_reason?.ru&&link.relation_family?.family_reason?.en)).toBe(true);
    expect(core.links.some(link=>link.relation_family.family&&link.relation_family.relation_type==='tos.relation.unmapped')).toBe(true);
    delete core.links[0].relation_family.family;
    expect(coreIssues(core,chapters,registry)).toContain(`link ${core.links[0].id}: incomplete subject-model family`);
  });

  it('retains earlier reviewed senses beside source-visible corrections',()=>{
    const core=fresh();
    for(const id of ['dwarf--recurrence--4','circle:link:04:nausea','shepherd--zarathustra--6']){
      const link=core.links.find(item=>item.id===id),prior=link.sense_revision_history?.[0];
      expect(link.sense_revision?.reviewer?.id).toBe('p4-independent-review');
      expect(prior?.sense?.ru).toBeTruthy();
      expect(prior.sense.ru).not.toBe(link.sense.ru);
      expect(prior.assessment?.reviewer).toBeTruthy();
    }
    const circle=core.links.find(item=>item.id==='circle:link:04:nausea');
    expect(circle.answer.ru).toBe(circle.sense.ru);
    expect(circle.answer.en).toBe(circle.sense.en);
    expect(circle.sense_revision_history[0].answer.ru).toContain('ведёт к');
    expect(circle.answer.cite).toEqual(circle.sense_revision_history[0].answer.cite);
  });

  it('keeps contextual use, voice and unreviewed forms separate at exact places',()=>{
    const core=fresh(),terms=new Map(core.semanticUses.terms.map(term=>[term.id,term]));
    expect([...terms.keys()].sort()).toEqual(['bruder','gold','schenken','segen','untergehen','wiederkunft']);
    const uses=terms.get('wiederkunft').occurrences;
    const nausea=uses.find(use=>use.semantic_use.group_id==='wiederkunft-nausea');
    const teacher=uses.find(use=>use.semantic_use.group_id==='wiederkunft-teacher');
    expect(nausea.voice.speaker).toBe('zarathustra');
    expect(teacher.voice.speaker).toBe('animals');
    expect(uses.some(use=>use.semantic_use.review_status==='unknown'&&use.semantic_use.meaning===null)).toBe(true);
    const unit='tos.text-unit.sid-2a1b29f9c62115040c9a0330cad6d16e';
    const gold=terms.get('gold').occurrences.find(use=>use.cite.u[0]===unit&&use.form==='Gold');
    const gift=terms.get('schenken').occurrences.find(use=>use.cite.u[0]===unit&&use.form==='schenkt');
    expect(gold.native_candidate.record_id).not.toBe(gift.native_candidate.record_id);
    expect(gold.native_candidate.status).toBe('not-materialized');
    teacher.semantic_use.meaning=null;
    expect(coreIssues(core,chapters,registry)).toContain(`semantic use ${teacher.id}: incomplete contextual reading`);
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

  it('checks new proposal citations and the voice and native type of exact-place comparisons',()=>{
    const core=fresh();
    core.proposed[0].reading.grounds[0].q='not in the witness';
    core.placeRelations.relations[0].from.speaker='zarathustra';
    core.placeRelations.relations[0].native_candidate.relation_type='tos.relation.authored-by';
    const problems=coreIssues(core,chapters,registry);
    expect(problems.some(issue=>issue.includes('not in the witness'))).toBe(true);
    expect(problems).toContain('place relation place:dwarf-rebuke from: attribution disagrees with voice map');
    expect(problems).toContain('place relation place:dwarf-rebuke: invalid native relation type');
    core.placeRelations.relations={};
    expect(coreIssues(core,chapters,registry)).toContain('place relations: invalid schema or relations');
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
