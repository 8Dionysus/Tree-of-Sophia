import {execFileSync} from 'node:child_process';
import {readFileSync} from 'node:fs';
import {join} from 'node:path';
import {describe,expect,it} from 'vitest';
import {ADMITTED,FIELD_SCHEMA,fieldCites,fieldIssues,fieldSubjects,libraryIssues,readField} from '../../../../bonsai/lib/field.mjs';
import {externalPlacesIssues} from '../../../../bonsai/lib/fragments.mjs';
import {checkBonsai,corpusOnDisk,loadBonsai,termPlaceReader} from '../../../../bonsai/lib/graft.mjs';
import {semanticRegistry} from '../../../../bonsai/lib/objects.mjs';
import {corpusReader} from '../../../../bonsai/lib/corpus-align.mjs';
import {formulaSpan} from '../../../../bonsai/review/aesthetics/formula-span.mjs';

const root=new URL('../../../../',import.meta.url).pathname;
const {modules}=loadBonsai(root,'aesthetics'),{field,library,identities,alignment,fragments}=modules;
const KANT='immanuel-kant.kritik-der-urteilskraft';
const registry=semanticRegistry(root),works=new Set(Object.keys(identities.works));
const issues=module=>fieldIssues(module,{registry,library,works});
const changed=change=>{const copy=structuredClone(field);change(copy);return issues(copy);};
const corpusFetched=corpusOnDisk(root)==='present';

describe('the Aesthetics field',()=>{
  it('refuses formula spans that start inside a sentence or end before their opening words',()=>{
    expect(()=>formulaSpan('A partial sentence.',{from:'partial',to:'sentence.'})).toThrow('opens inside a sentence');
    expect(()=>formulaSpan('First. Second sentence.',{from:'First. Second',to:'First.'})).toThrow('end before the opening words');
    expect(formulaSpan('😀 First. Second sentence.',{from:'Second',to:'sentence.'})).toEqual({s:9,e:25});
  });
  it('keeps its contract against the Tree\'s registries',()=>{
    expect(field.schema).toBe(FIELD_SCHEMA);
    expect(issues(field)).toEqual([]);
    expect(libraryIssues(library)).toEqual([]);
  });
  it('holds thirty-six areas in four groups, each with its question in both languages',()=>{
    expect(field.areas.map(area=>area.n)).toEqual(Array.from({length:36},(_,i)=>i+1));
    expect(field.groups.map(group=>[group.id,group.areas.length])).toEqual([['A',12],['B',8],['C',8],['D',8]]);
    for(const area of field.areas){expect(area.question.ru,area.id).toMatch(/\?$/);expect(area.question.en,area.id).toMatch(/\?$/);}
    expect(readField(field).area('area-beautiful').question.ru).toBe('Что и на каком основании переживается и оценивается как красивое?');
  });
  it('types the twelve categories of the first group as categories and leaves every other concept a candidate',()=>{
    const categories=field.concepts.filter(concept=>concept.type==='tos.entity.philosophical-category');
    expect(categories.map(concept=>concept.id)).toEqual(['the-beautiful','the-sublime','the-ugly','the-base','the-tragic','the-comic','the-grotesque','the-uncanny-and-the-horrible','the-absurd','the-picturesque','the-cute','the-interesting']);
    expect(new Set(field.concepts.filter(concept=>!categories.includes(concept)).map(concept=>concept.type))).toEqual(new Set(['tos.entity.concept']));
    expect(field.concepts).toHaveLength(407);
  });
  it('lets a conception in only with its place cited in its own work, a verdict of a named reader and the records it grew from',()=>{
    expect(field.conceptions.length).toBeGreaterThan(0);
    for(const conception of field.conceptions){
      expect(conception.formula.cite.length,conception.id).toBeGreaterThan(0);
      for(const cite of conception.formula.cite)expect(cite.w,conception.id).toBe(conception.work);
      expect(ADMITTED[conception.admission],conception.id).toContain(conception.reading.verdict);
      expect(field.readers.map(reader=>reader.id),conception.id).toContain(conception.reading.reader);
    }
    expect(changed(copy=>{delete copy.conceptions[0].formula;})).toContain('conception plato.symposium:ascent: no formula cited in the work');
    expect(changed(copy=>{copy.conceptions[0].formula.cite[0].w='aristotle.poetics';})).toContain('conception plato.symposium:ascent: the formula is cited in aristotle.poetics, not in its work');
    expect(changed(copy=>{copy.conceptions[0].reading.verdict='undecided';})).toContain('conception plato.symposium:ascent: map does not rest on the verdict undecided');
    expect(changed(copy=>{copy.conceptions[0].reading.reader='nobody';})).toContain('conception plato.symposium:ascent: no reader the field names');
    expect(changed(copy=>{copy.conceptions[0].grew_from=[];})).toContain('conception plato.symposium:ascent: names no research record it grew from');
    expect(changed(copy=>{copy.conceptions[0].of=['no-such-concept'];})).toContain('conception plato.symposium:ascent: unknown concept no-such-concept');
    expect(changed(copy=>{delete copy.conceptions[0].thesis.en;})).toContain('conception plato.symposium:ascent: no thesis in both languages');
    expect(changed(copy=>{copy.conceptions[0].work='plato.phaedrus';})).toContain('conception plato.symposium:ascent: its work plato.phaedrus is not in the library');
    expect(changed(copy=>{copy.conceptions[0].agents=['aristotle'];})).toContain('conception plato.symposium:ascent: authors differ from its work plato.symposium');
  });
  it('rests every link on conceptions it holds and names it by a verb',()=>{
    for(const link of field.links)expect(link.grounds.every(ground=>field.conceptions.some(conception=>conception.id===ground)),link.id).toBe(true);
    expect(changed(copy=>{copy.links[0].grounds=['plato.symposium:nothing'];})).toContain('link eros:leads-up-to:the-beautiful-itself: unknown ground plato.symposium:nothing');
    expect(changed(copy=>{copy.links[0].to='nowhere';})).toContain('link eros:leads-up-to:the-beautiful-itself: unknown end nowhere');
    expect(changed(copy=>{delete copy.links[0].verb.ru;})).toContain('link eros:leads-up-to:the-beautiful-itself: no verb in both languages');
    expect(changed(copy=>{copy.links[0].relation='tos.relation.conception-of';})).toContain('link eros:leads-up-to:the-beautiful-itself: tos.entity.concept is outside the domain of tos.relation.conception-of');
    expect(changed(copy=>{copy.conceptions[0].reading.verdict='undecided';copy.conceptions[0].admission='held';})).toContain('link the-ascent:ends-in:the-beautiful-itself: is on the map while none of its grounds is');
  });
  it('refuses an area without a question and a concept outside every area',()=>{
    expect(changed(copy=>{delete copy.areas[0].question;})).toContain('area area-beautiful: no question in both languages');
    expect(changed(copy=>{copy.concepts[0].areas=[];})).toContain('concept the-beautiful: stands in no area');
    expect(changed(copy=>{copy.concepts[1].id=copy.concepts[0].id;})).toContain('concept the-beautiful: missing or duplicate id');
  });
  it('reads as a stand will read it: a concept\'s admitted conceptions and the links at a concept',()=>{
    const read=readField(field);
    expect(read.conceptsOf('area-beautiful').length).toBe(39);
    expect(read.conceptionsOf('the-beautiful').map(conception=>conception.work)).toEqual(expect.arrayContaining(['plato.symposium','plotinus.enneads','aristotle.poetics']));
    expect(read.linksOf('symmetry').map(link=>link.id)).toEqual(['the-beautiful:is-other-than:symmetry']);
    const subjects=fieldSubjects(field,library);
    for(const id of ['area-beautiful','the-beautiful','plato.symposium:ascent','plato','plato.symposium'])expect(subjects.has(id),id).toBe(true);
  });
  it('cites a work the corpus does not hold by places with sentence-like outer punctuation',()=>{
    expect(externalPlacesIssues(fragments)).toEqual([]);
    const read=termPlaceReader(root,alignment,fragments,{corpus:'absent'}),cites=fieldCites(field).filter(({cite})=>cite.loc);
    expect(cites.length).toBeGreaterThan(40);
    for(const {conception,cite} of cites){
      const place=read(cite,fragments.works[cite.w].original);
      expect(place.error,`${conception} ${cite.loc}`).toBeUndefined();
      const texts=fragments.places[cite.w][cite.loc].texts;
      expect(texts[fragments.works[cite.w].original],cite.loc).toMatch(/^\p{Lu}[^]*[.!?]$/u);
      for(const text of Object.values(texts))expect(text,cite.loc).toMatch(/^\p{Lu}/u);
    }
    // Every place the module holds is the formula of a conception: the bonsai keeps no text it does not cite.
    for(const work of Object.keys(fragments.places))expect(new Set(cites.filter(({cite})=>cite.w===work).map(({cite})=>cite.loc)),work).toEqual(new Set(Object.keys(fragments.places[work])));
    expect(Object.keys(fragments.places)).toEqual([KANT,'edmund-burke.a-philosophical-enquiry-into-the-origin-of-our']);
    const broken=structuredClone(field),kant=broken.conceptions.find(conception=>conception.work===KANT);
    kant.formula.cite[0].loc='§1 no such lines';
    expect(checkBonsai(root,'aesthetics',{overrides:{field:broken}}).problems).toContain(`field: conception ${kant.id}: no place ${KANT} §1 no such lines among the places of other works`);
  });
  it('keeps the words of the printed page where a transcription differs from it',()=>{
    expect(fragments.provenance.corrections.map(correction=>correction.to)).toEqual(['unterliegen müßte. Auf solche Weise','Angenehm heißt Jemandem das','smoothness; a quality so essential']);
    for(const {from,to,lang,place} of fragments.provenance.corrections){
      const work=Object.keys(fragments.places).find(id=>place.startsWith(`${id} `)),text=fragments.places[work][place.slice(work.length+1)].texts[lang];
      expect(text,place).toContain(to);expect(text,place).not.toContain(from);
    }
    for(const work of Object.values(fragments.works))for(const witness of Object.values(work.witnesses))expect(witness.rights.basis).toBe('public-domain');
  });
  it('holds back what its own reader made until another reader has read the place',()=>{
    const read=readField(field),division=field.conceptions.find(conception=>conception.id===`${KANT}:division-of-the-sublime`);
    expect([division.reading.verdict,division.admission]).toEqual(['undecided','held']);
    expect(read.conceptionsOf('the-sublime').map(conception=>conception.id)).not.toContain(division.id);
    expect(read.conceptionsOf('the-sublime').map(conception=>conception.id)).toContain(`${KANT}:absolutely-great`);
    expect(read.linksOf('the-mathematically-sublime').map(link=>link.id)).toEqual(['the-mathematically-sublime:is-absolute:greatness']);
    const held=field.links.filter(link=>link.admission==='held');
    expect(held.map(link=>link.id)).toEqual(['the-sublime:divides-into:the-mathematically-sublime','the-sublime:divides-into:the-dynamically-sublime','terror:is-the-ruling-principle-of:the-sublime']);
    expect(new Set(held.map(link=>link.reading.verdict))).toEqual(new Set(['undecided']));
  });
  it.skipIf(!corpusFetched)('cites formulas that run over whole sentences of the original',()=>{
    const read=termPlaceReader(root,alignment,null,{corpus:'present'});
    const readCorpus=corpusReader(join(root,'access/web/public/public-corpus'));
    for(const {conception,cite} of fieldCites(field).filter(({cite})=>cite.u)){
      const work=alignment.works[cite.w],place=read(cite,work.original.language);
      expect(place.error,conception).toBeUndefined();
      expect(place.text.trim(),conception).toMatch(/[.;]$/u);
      const version=readCorpus(work.documentId).versions.find(item=>item.id===work.original.versionId);
      const unit=version.units.find(item=>item.id===cite.u[0]);
      const before=Array.from(unit.text).slice(0,cite.s??0).join('').trimEnd();
      expect(before===''||/[.;!?]$/u.test(before),conception).toBe(true);
    }
    const ascent=field.conceptions.find(conception=>conception.id==='plato.symposium:ascent');
    expect(read(ascent.formula.cite[0],'grc').text).toMatch(/^τοῦτο γὰρ δή ἐστι τὸ ὀρθῶς ἐπὶ τὰ ἐρωτικὰ ἰέναι.*ὃ ἔστι καλόν\.$/u);
    const broken=structuredClone(field);broken.conceptions[0].formula.cite[0].e=10**6;
    expect(checkBonsai(root,'aesthetics',{overrides:{field:broken}}).problems).toContain('field: conception plato.symposium:ascent: invalid offsets for plato.symposium');
  });
  it.skipIf(!corpusFetched)('is what the planting tool gives from the layer and the readers\' records',()=>{
    const out=execFileSync(process.execPath,[join(root,'bonsai/review/aesthetics/plant.mjs'),'--check'],{encoding:'utf8'});
    expect(out).toBe('field.json: matches what the records give\nlibrary.json: matches what the records give\nidentities.json: matches what the records give\n');
  });
  it('keeps every concept and conception traceable to the research layer',()=>{
    const layer=name=>JSON.parse(readFileSync(join(root,'bonsai/review/aesthetics/layer',`${name}.json`),'utf8'));
    const concepts=new Set(layer('concepts').flatMap(concept=>concept.keys)),records=new Set([...layer('readings'),...layer('links')].map(record=>record.key));
    for(const concept of field.concepts)for(const key of concept.grew_from)expect(concepts.has(key),`${concept.id} ${key}`).toBe(true);
    for(const record of [...field.conceptions,...field.links])for(const key of record.grew_from)expect(records.has(key),`${record.id} ${key}`).toBe(true);
  });
});
