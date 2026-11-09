// The core of a field of thought: bonsai/CORE.md, "A core across works".
//
// `field.json` (tos.bonsai.field.v1) holds a discipline cut across many works.
// Its areas each ask a question; its concepts are the subjects any author can
// address; a conception is one work's answer at a named place, in the voice
// that gives it, with one line of ours beside the work's own words; a link
// joins concepts or conceptions by a verb and rests on conceptions. A
// conception enters only with its place cited in the original and with the
// verdict of a reader who read that place, and it keeps the keys of the
// research records it grew from.
//
// `library.json` (tos.bonsai.library.v1) names the works and authors the
// conceptions rest on. Citations name a work by its library ID.

export const FIELD_SCHEMA='tos.bonsai.field.v1';
export const LIBRARY_SCHEMA='tos.bonsai.library.v1';
/** What a conception or a link claims: the work says it at the place, it holds through several places, or it is offered for testing. */
export const CLAIMED=Object.freeze(['text','reconstruction','hypothesis']);
export const VERDICTS=Object.freeze(['confirmed','reconstruction','revise','rejected','undecided']);
/** The verdicts each admission of the stand may rest on. */
export const ADMITTED=Object.freeze({map:['confirmed','reconstruction','revise'],held:['undecided'],rejected:['rejected']});
const AREA_TYPE='tos.entity.navigation-region',CONCEPT_TYPES=['tos.entity.concept'],CONCEPTION_TYPE='tos.entity.conception';
const bilingual=value=>typeof value?.ru==='string'&&!!value.ru.trim()&&typeof value?.en==='string'&&!!value.en.trim();
const ID=/^[a-z0-9]+(?:[-.:/][a-z0-9]+)*$/;
const RECORD=/^[A-D]:\S+$/;

/** Problems of a library: every work and author named in both languages, every work with authors the library holds. */
export function libraryIssues(module){
  const problems=[];
  if(module?.schema!==LIBRARY_SCHEMA)problems.push(`schema is not ${LIBRARY_SCHEMA}`);
  const agents=module?.agents??{},works=module?.works??{};
  for(const [id,agent] of Object.entries(agents)){
    if(!ID.test(id))problems.push(`agent ${id}: id`);
    if(!bilingual(agent.name))problems.push(`agent ${id} has no name in both languages`);
  }
  for(const [id,work] of Object.entries(works)){
    if(!ID.test(id))problems.push(`work ${id}: id`);
    if(!bilingual(work.title))problems.push(`work ${id} has no title in both languages`);
    if(typeof work.original!=='string'||!work.original)problems.push(`work ${id} names no original language`);
    if(!Array.isArray(work.agents)||!work.agents.length)problems.push(`work ${id} names no author`);
    for(const agent of work.agents??[])if(!agents[agent])problems.push(`work ${id}: unknown author ${agent}`);
  }
  return problems;
}

/** The assessment every admitted claim carries: what is claimed, the reader's verdict and the stand's admission resting on it. */
function assessmentIssues(at,record,readers){
  const problems=[];
  if(!CLAIMED.includes(record.status))problems.push(`${at}: status ${record.status}`);
  if(!record.reading||!readers.has(record.reading.reader))problems.push(`${at}: no reader the field names`);
  if(!VERDICTS.includes(record.reading?.verdict))problems.push(`${at}: verdict ${record.reading?.verdict}`);
  if(!ADMITTED[record.admission])problems.push(`${at}: admission ${record.admission}`);
  else if(!ADMITTED[record.admission].includes(record.reading?.verdict))problems.push(`${at}: ${record.admission} does not rest on the verdict ${record.reading?.verdict}`);
  if(!Array.isArray(record.grew_from)||!record.grew_from.length||record.grew_from.some(key=>!RECORD.test(key)))problems.push(`${at}: names no research record it grew from`);
  return problems;
}

/**
 * Problems of a field. `registry` is the Tree's entity and relation registries, `library` the bonsai's library,
 * `works` the IDs a citation may name (the graft map's and the places of other works'), `placeIssues` the check
 * of one cited place; each is used where given.
 */
export function fieldIssues(module,{registry=null,library=null,works=null,placeIssues=null}={}){
  const problems=[];
  if(module?.schema!==FIELD_SCHEMA)problems.push(`schema is not ${FIELD_SCHEMA}`);
  if(!bilingual(module?.title))problems.push('no title in both languages');
  if(!bilingual(module?.posture))problems.push('no posture in both languages');
  const readers=new Set((module?.readers??[]).map(reader=>reader.id));
  const typed=(at,type,within)=>{
    if(!registry)return;
    if(!registry.hasType(type))problems.push(`${at}: ${type} is not in the Tree's entity registry`);
    else if(!registry.within(type,within))problems.push(`${at}: ${type} is outside ${within.join(', ')}`);
  };

  const areas=new Map(),numbers=new Set();
  for(const area of module?.areas??[]){
    const at=`area ${area.id}`;
    if(!ID.test(area.id??'')||areas.has(area.id))problems.push(`${at}: missing or duplicate id`);
    areas.set(area.id,area);
    if(!Number.isInteger(area.n)||numbers.has(area.n))problems.push(`${at}: missing or duplicate number`);
    numbers.add(area.n);
    if(!(module?.groups??[]).some(group=>group.id===area.group))problems.push(`${at}: unknown group ${area.group}`);
    if(area.type!==AREA_TYPE)problems.push(`${at}: type ${area.type}`);
    typed(at,area.type,[AREA_TYPE]);
    if(!bilingual(area.title))problems.push(`${at}: no title in both languages`);
    // The question belongs to the Tree: an area without one is a heading, and the field keeps none.
    if(!bilingual(area.question))problems.push(`${at}: no question in both languages`);
    if(!/^AE\d{2}$/.test(area.grew_from??''))problems.push(`${at}: names no area of the research it grew from`);
  }

  const concepts=new Map();
  for(const concept of module?.concepts??[]){
    const at=`concept ${concept.id}`;
    if(!ID.test(concept.id??'')||concepts.has(concept.id)||areas.has(concept.id))problems.push(`${at}: missing or duplicate id`);
    concepts.set(concept.id,concept);
    typed(at,concept.type,CONCEPT_TYPES.concat('tos.entity.philosophical-category'));
    if(!bilingual(concept.title))problems.push(`${at}: no title in both languages`);
    if(!Array.isArray(concept.areas)||!concept.areas.length)problems.push(`${at}: stands in no area`);
    for(const area of concept.areas??[])if(!areas.has(area))problems.push(`${at}: unknown area ${area}`);
    if(!Array.isArray(concept.terms)||concept.terms.some(term=>typeof term!=='string'||!term.trim()))problems.push(`${at}: terms`);
    if(!Array.isArray(concept.grew_from)||!concept.grew_from.length||concept.grew_from.some(key=>!RECORD.test(key)))problems.push(`${at}: names no research record it grew from`);
  }

  const conceptions=new Map();
  for(const conception of module?.conceptions??[]){
    const at=`conception ${conception.id}`;
    if(!ID.test(conception.id??'')||conceptions.has(conception.id)||concepts.has(conception.id)||areas.has(conception.id))problems.push(`${at}: missing or duplicate id`);
    conceptions.set(conception.id,conception);
    if(conception.type!==CONCEPTION_TYPE)problems.push(`${at}: type ${conception.type}`);
    typed(at,conception.type,[CONCEPTION_TYPE]);
    if(!Array.isArray(conception.of)||!conception.of.length)problems.push(`${at}: is a conception of no concept`);
    for(const concept of conception.of??[])if(!concepts.has(concept))problems.push(`${at}: unknown concept ${concept}`);
    if(!Array.isArray(conception.areas)||!conception.areas.length)problems.push(`${at}: stands in no area`);
    for(const area of conception.areas??[])if(!areas.has(area))problems.push(`${at}: unknown area ${area}`);
    if(library){
      const work=library.works?.[conception.work];
      if(!work)problems.push(`${at}: its work ${conception.work} is not in the library`);
      if(!Array.isArray(conception.agents)||!conception.agents.length)problems.push(`${at}: names no author`);
      for(const agent of conception.agents??[])if(!library.agents?.[agent])problems.push(`${at}: unknown author ${agent}`);
    }
    if(conception.voice!==undefined&&!bilingual(conception.voice))problems.push(`${at}: voice in one language`);
    if(typeof conception.locus!=='string'||!conception.locus.trim())problems.push(`${at}: no locator`);
    // The work's own words are the centre: a conception without its place in the original is a paraphrase and stays in the research layer.
    const cites=[].concat(conception.formula?.cite??[]);
    if(!cites.length)problems.push(`${at}: no formula cited in the work`);
    for(const cite of cites){
      if(cite?.w!==conception.work)problems.push(`${at}: the formula is cited in ${cite?.w}, not in its work`);
      if(works&&!works.has(cite?.w))problems.push(`${at}: neither the graft map nor the places of other works hold ${cite?.w}`);
      if(placeIssues)for(const issue of placeIssues(cite))problems.push(`${at}: ${issue}`);
    }
    if(!bilingual(conception.thesis))problems.push(`${at}: no thesis in both languages`);
    if(conception.distinction!==undefined&&!bilingual(conception.distinction))problems.push(`${at}: distinction in one language`);
    problems.push(...assessmentIssues(at,conception,readers));
  }

  const links=new Set(),typeOf=id=>concepts.get(id)?.type??conceptions.get(id)?.type??null;
  for(const link of module?.links??[]){
    const at=`link ${link.id}`;
    if(!ID.test(link.id??'')||links.has(link.id))problems.push(`${at}: missing or duplicate id`);
    links.add(link.id);
    for(const end of [link.from,link.to])if(!typeOf(end))problems.push(`${at}: unknown end ${end}`);
    if(!bilingual(link.verb))problems.push(`${at}: no verb in both languages`);
    // A link rests on conceptions the field holds: both ends are shown by places of works.
    if(!Array.isArray(link.grounds)||!link.grounds.length)problems.push(`${at}: rests on no conception`);
    for(const ground of link.grounds??[])if(!conceptions.has(ground))problems.push(`${at}: unknown ground ${ground}`);
    if(link.relation!==undefined&&registry){
      const relation=registry.relation(link.relation);
      if(!relation)problems.push(`${at}: ${link.relation} is not in the Tree's relation registry`);
      else{
        if(typeOf(link.from)&&!registry.within(typeOf(link.from),relation.domain_type_ids))problems.push(`${at}: ${typeOf(link.from)} is outside the domain of ${link.relation}`);
        if(typeOf(link.to)&&!registry.within(typeOf(link.to),relation.range_type_ids))problems.push(`${at}: ${typeOf(link.to)} is outside the range of ${link.relation}`);
      }
    }
    problems.push(...assessmentIssues(at,link,readers));
  }
  return problems;
}

/** Every citation a field makes, with the conception that makes it. */
export function fieldCites(module){
  return (module?.conceptions??[]).flatMap(conception=>[].concat(conception.formula?.cite??[]).map(cite=>({conception:conception.id,cite})));
}

/** What a medium may be bound to in a field: its areas, concepts and conceptions, and the library's authors and works. */
export function fieldSubjects(module,library=null){
  return new Set([...(module?.areas??[]),...(module?.concepts??[]),...(module?.conceptions??[])].map(record=>record.id)
    .concat(Object.keys(library?.agents??{}),Object.keys(library?.works??{})));
}

/** A field as a stand reads it: its areas in order, the concepts of an area, a concept's admitted conceptions and the admitted links at a record. */
export function readField(module){
  const areas=[...(module?.areas??[])].sort((a,b)=>a.n-b.n),concepts=module?.concepts??[],conceptions=module?.conceptions??[];
  return {
    areas,
    area:id=>areas.find(area=>area.id===id)??null,
    conceptsOf:area=>concepts.filter(concept=>concept.areas.includes(area)),
    concept:id=>concepts.find(concept=>concept.id===id)??null,
    // The order of works in time belongs to the bonsai's chronology; until it is kept, conceptions stand as they were planted.
    conceptionsOf:concept=>conceptions.filter(conception=>conception.admission==='map'&&conception.of.includes(concept)),
    linksOf:id=>(module?.links??[]).filter(link=>link.admission==='map'&&(link.from===id||link.to===id)),
  };
}
