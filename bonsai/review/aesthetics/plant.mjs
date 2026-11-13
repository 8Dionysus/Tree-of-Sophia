#!/usr/bin/env node
// Plants the Aesthetics bonsai from the research layer and the readers' records.
//
//   node bonsai/review/aesthetics/plant.mjs            # write bonsai/aesthetics/{field,library,identities}.json
//   node bonsai/review/aesthetics/plant.mjs --check    # say whether the stored modules are what the records give
//
// The areas and concepts come from layer/ and inputs/. A conception comes only from a record under readings/:
// a reader's account of one place, read in the work itself. For a work of the public corpus the record names the
// formula by the words that open and close it in a corpus unit; this tool finds the span, refuses one that does
// not run over whole sentences, and writes the citation. Fetch the corpus first:
// node bonsai/lib/layers.mjs fetch access/public-corpus. For a work the corpus does not hold, the record names a
// place of bonsai/aesthetics/fragments.json, whose text in the original is the formula; this tool refuses a place
// the module does not hold or one lacking sentence-like outer punctuation; a reader must verify boundaries against the source.
// An accepted packet of the planting line (planting/packets/<work>/) brings its reading as one more record and
// its places into fragments.json; `--dry [--packet NAME]` builds everything, checks it and writes nothing.

import {readFileSync,writeFileSync,readdirSync,existsSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {corpusReader} from '../../lib/corpus-align.mjs';
import {FIELD_SCHEMA,LIBRARY_SCHEMA,fieldIssues,libraryIssues} from '../../lib/field.mjs';
import {externalPlacesIssues,rightsBases} from '../../lib/fragments.mjs';
import {formulaSpan} from './formula-span.mjs';

const here=dirname(fileURLToPath(import.meta.url)),root=join(here,'..','..','..'),home=join(root,'bonsai','aesthetics');
const json=path=>JSON.parse(readFileSync(path,'utf8'));
const layer=name=>json(join(here,'layer',`${name}.json`)),input=name=>json(join(here,'inputs',`${name}.json`));
const slug=text=>text.toLowerCase().normalize('NFKD').replace(/\p{M}/gu,'').replace(/[’']s\b/g,'s').replace(/[^a-z0-9]+/g,'-').replace(/^-|-$/g,'');
const capital=text=>text.charAt(0).toUpperCase()+text.slice(1);
const fail=message=>{console.error(message);process.exit(1);};

const areasIn=input('areas').areas,titles=input('concepts').titles,concepts=layer('concepts'),agents=layer('agents'),works=layer('works');
const areaId=Object.fromEntries(areasIn.map(area=>[area.id,`area-${slug(area.title.en).replace(/^the-/,'')}`]));
const conceptId=Object.fromEntries(concepts.map(concept=>{if(!titles[concept.key])fail(`concept ${concept.key} has no English title in inputs/concepts.json`);return [concept.key,slug(titles[concept.key])];}));
// The twelve categories of group A stand in their registers as categories; every other concept is a candidate until a reading types it.
const CATEGORY=/^(категория \/ навигационный предмет)$/;

const records=readdirSync(join(here,'readings')).filter(name=>name.endsWith('.json')).sort().map(name=>({name,...json(join(here,'readings',name))}));
const corpusDir=join(root,'access/web/public/public-corpus'),read=existsSync(join(corpusDir,'manifest.json'))?corpusReader(corpusDir):null;
const fragments=existsSync(join(home,'fragments.json'))?json(join(home,'fragments.json')):null;

/** The planting line (planting/AGENTS.md): one work made by one reader and read by a second, kept as a packet. An accepted
 *  packet is planted: its reading joins the records, its places and their provenance join the places module. `--packet NAME`
 *  reads one more packet as if it were accepted, so that a reader can try the planting before anyone accepts it. */
const packetsDir=join(here,'planting','packets'),tried=process.argv.includes('--packet')?process.argv[process.argv.indexOf('--packet')+1]:null;
const packets=(existsSync(packetsDir)?readdirSync(packetsDir,{withFileTypes:true}).filter(entry=>entry.isDirectory()).map(entry=>entry.name).sort():[])
  .filter(name=>existsSync(join(packetsDir,name,'accepted.json'))||name===tried||(()=>{const dir=join(packetsDir,name,'witnesses');return existsSync(dir)&&readdirSync(dir).some(file=>file.endsWith('.json.accepted'));})());
const seconds=[],whole=[];
// Remove the previous translation projection before rebuilding it from reviewed packets.
for(const [name,languages] of Object.entries(fragments.provenance.translation_packets??{})){
  const work=fragments.works[name];if(!work)continue;
  for(const [lang,previous] of Object.entries(languages)){
    delete work.witnesses?.[lang];
    if(previous.absent){work.absent??=[];if(!work.absent.some(item=>item.language===lang))work.absent.push(previous.absent);}
    if(previous.unsearched){work.unsearched??=[];if(!work.unsearched.some(item=>item.language===lang))work.unsearched.push(previous.unsearched);}
    for(const place of Object.values(fragments.places[name]??{})){
      delete place.texts?.[lang];delete place.links?.[lang];delete place.quotation_justification?.[lang];
      if(place.refs)place.refs=place.refs.filter(ref=>ref[0]!==lang);
      if(place.quotation_justification&&!Object.keys(place.quotation_justification).length)delete place.quotation_justification;
    }
  }
}
const translationPackets={};
// Extension-only places are rebuilt from the accepted extension packets.
for(const [name,places] of Object.entries(fragments.provenance.extension_places??{})){
  for(const [loc,previous] of Object.entries(places)){
    if(previous===null)delete fragments.places?.[name]?.[loc];
    else if(fragments.places?.[name])fragments.places[name][loc]=previous;
  }
}
const extensionPlaces={};
for(const correction of fragments.provenance.extension_corrections??[]){
  const at=fragments.provenance.corrections.findIndex(item=>JSON.stringify(item)===JSON.stringify(correction));
  if(at>=0)fragments.provenance.corrections.splice(at,1);
}
for(const [name,previous] of Object.entries(fragments.provenance.extension_checked??{})){
  if(previous===null)delete fragments.provenance.checked_on_the_page_image[name];
  else fragments.provenance.checked_on_the_page_image[name]=previous;
}
const extensionCorrections=[],extensionChecked={};
// A work a packet brought whole leaves the places module again when that packet is no longer accepted.
for(const name of fragments.provenance.packets??[])if(!packets.includes(name)){
  delete fragments.works[name];delete fragments.places[name];delete fragments.provenance.checked_on_the_page_image[name];
  fragments.provenance.corrections=fragments.provenance.corrections.filter(correction=>!correction.place.startsWith(`${name} `));
}
for(const name of packets){
  const at=join(packetsDir,name),path=`bonsai/review/aesthetics/planting/packets/${name}`;
  const hasReading=existsSync(join(at,'accepted.json'))||name===tried;
  const places=hasReading?json(join(at,'places.json')):{entry:null,places:{},corrections:[]};
  if(hasReading){
    records.push({name:`planting/packets/${name}/reading.json`,path:`${path}/reading.json`,...json(join(at,'reading.json'))});
    if(existsSync(join(at,'review.json'))){const {reader}=json(join(at,'review.json'));seconds.push({...reader,record:`${path}/review.json`});}
  }
  if(places.entry){fragments.works[name]=places.entry;whole.push(name);}
  if(!fragments.works[name])fail(`packet ${name}: no entry for a work the places module does not hold`);
  fragments.places[name]={...(places.entry?{}:fragments.places[name]),...places.places};
  // Each accepted translation is an independent source record; the acceptance digest
  // binds the second reader's decision to the exact file consumed here.
  const witnessesDir=join(at,'witnesses');
  for(const file of existsSync(witnessesDir)?readdirSync(witnessesDir).filter(file=>file.endsWith('.json')).sort():[]){
    const full=join(witnessesDir,file),accept=`${full}.accepted`;
    if(!existsSync(accept))continue;
    const raw=readFileSync(full),record=JSON.parse(raw),receipt=json(accept),lang=file.slice(0,-5);
    if(receipt.sha256!==createHash('sha256').update(raw).digest('hex')||record.language!==lang)fail(`packet ${name}: ${lang} witness changed since review`);
    if(lang===fragments.works[name].original||fragments.works[name].witnesses?.[lang])fail(`packet ${name}: witness ${lang} already exists`);
    fragments.works[name].witnesses[lang]=record.witness;
    const absent=(fragments.works[name].absent??[]).find(item=>item.language===lang)??null;
    const unsearched=(fragments.works[name].unsearched??[]).find(item=>item.language===lang)??null;
    (translationPackets[name]??={})[lang]={absent,unsearched};
    fragments.works[name].absent=(fragments.works[name].absent??[]).filter(item=>item.language!==lang);
    if(fragments.works[name].unsearched)fragments.works[name].unsearched=fragments.works[name].unsearched.filter(item=>item.language!==lang);
    for(const [loc,translated] of Object.entries(record.places??{})){
      const place=fragments.places[name]?.[loc];if(!place)fail(`packet ${name}: translation ${lang} names unknown place ${loc}`);
      place.texts[lang]=translated.text;place.links??={};place.links[lang]=translated.url;
      place.refs??=[];place.refs.push([lang,`${record.witness.edition}, ${translated.page}`]);
      if(translated.justification){place.quotation_justification??={};place.quotation_justification[lang]=translated.justification;}
    }
  }
  const own=new Set(Object.keys(places.places).map(loc=>`${name} ${loc}`)),seen=fragments.provenance.checked_on_the_page_image;
  fragments.provenance.corrections=[...fragments.provenance.corrections.filter(correction=>!own.has(correction.place)),...(places.corrections??[])];
  if(places.checked_on_the_page_image?.length)seen[name]=[...new Set([...(places.entry?[]:seen[name]??[]),...places.checked_on_the_page_image])];
  const extensionsDir=join(at,'extensions');
  for(const extension of existsSync(extensionsDir)?readdirSync(extensionsDir,{withFileTypes:true}).filter(item=>item.isDirectory()).map(item=>item.name).sort():[]){
    const ext=join(extensionsDir,extension),selector=`${name}/extensions/${extension}`;
    if(!existsSync(join(ext,'accepted.json'))&&tried!==selector)continue;
    const extensionPath=`${path}/extensions/${extension}`,extensionPlacesFile=json(join(ext,'places.json'));
    if(extensionPlacesFile.work!==name||extensionPlacesFile.entry)fail(`extension ${selector}: wrong work or duplicate entry`);
    records.push({name:`planting/packets/${selector}/reading.json`,path:`${extensionPath}/reading.json`,...json(join(ext,'reading.json'))});
    if(existsSync(join(ext,'review.json'))){const {reader}=json(join(ext,'review.json'));seconds.push({...reader,record:`${extensionPath}/review.json`});}
    for(const [loc,value] of Object.entries(extensionPlacesFile.places??{})){
      const existing=fragments.places[name]?.[loc],lang=fragments.works[name].original;
      if(existing&&existing.texts?.[lang]!==value.texts?.[lang])fail(`extension ${selector}: ${loc} changes an existing original place`);
      if(!existing){(extensionPlaces[name]??={})[loc]=null;fragments.places[name][loc]=value;}
    }
    fragments.provenance.corrections.push(...(extensionPlacesFile.corrections??[]));
    extensionCorrections.push(...(extensionPlacesFile.corrections??[]));
    if(extensionPlacesFile.checked_on_the_page_image?.length){
      if(!(name in extensionChecked))extensionChecked[name]=seen[name]?[...seen[name]]:null;
      seen[name]=[...new Set([...(seen[name]??[]),...extensionPlacesFile.checked_on_the_page_image])];
    }
  }
}
if(whole.length||fragments.provenance.packets)fragments.provenance.packets=whole;
if(Object.keys(translationPackets).length)fragments.provenance.translation_packets=translationPackets;else delete fragments.provenance.translation_packets;
if(Object.keys(extensionPlaces).length)fragments.provenance.extension_places=extensionPlaces;else delete fragments.provenance.extension_places;
if(extensionCorrections.length)fragments.provenance.extension_corrections=extensionCorrections;else delete fragments.provenance.extension_corrections;
if(Object.keys(extensionChecked).length)fragments.provenance.extension_checked=extensionChecked;else delete fragments.provenance.extension_checked;

/** The citation of a formula: the unit a record names by its reference and the span between the opening and closing words. */
function cite(record,conception,formula,libraryWork){
  const at=`${record.name} ${conception.id}`;
  if(libraryWork.places==='fragments'){
    const text=fragments?.places?.[conception.work]?.[formula.place]?.texts?.[libraryWork.original];
    if(fragments?.works?.[conception.work]?.original!==libraryWork.original)fail(`${at}: fragments.json does not hold ${conception.work} in ${libraryWork.original}`);
    if(typeof text!=='string'){
      const place=fragments?.places?.[conception.work]?.[formula.place],witness=fragments?.works?.[conception.work]?.witnesses?.[libraryWork.original];
      const bases=rightsBases(witness?.rights);
      if(place?.url&&bases.length===3&&bases.every(basis=>basis==='locator-only'))return {w:conception.work,loc:formula.place};
      fail(`${at}: fragments.json holds no place ${JSON.stringify(formula.place)} of ${conception.work} in the original`);
    }
    if(!/^[\p{Lu}„“"(]/u.test(text)||!/[.!?][)”"]?$/u.test(text.trimEnd()))fail(`${at}: the place ${formula.place} does not run over whole sentences`);
    return {w:conception.work,loc:formula.place};
  }
  if(formula.place)fail(`${at}: a place is named for a work the record keeps in the corpus`);
  const work=read?.(libraryWork.tree);
  if(!work)fail(`${at}: the corpus does not hold ${libraryWork.tree}; fetch it (node bonsai/lib/layers.mjs fetch access/public-corpus)`);
  const original=work.versions.find(version=>version.language===libraryWork.original);
  const units=original.units.filter(unit=>unit.reference===formula.unit);
  if(units.length!==1)fail(`${at}: ${units.length} units of the original are named ${formula.unit}`);
  let span;try{span=formulaSpan(units[0].text,formula);}catch(error){fail(`${at}: ${error.message}`);}
  return {w:conception.work,u:[units[0].id,units[0].id],...span};
}

const library={schema:LIBRARY_SCHEMA,bonsai:'aesthetics',
  posture:{ru:'Произведения и авторы, на места которых опираются трактовки бонсая. Запись появляется здесь вместе с первой трактовкой, прочитанной в самом произведении.',en:'The works and authors whose places the bonsai’s conceptions rest on. A record enters with the first conception read in the work itself.'},
  agents:{},works:{}};
const identities={schema:'tos.bonsai.identities.v1',bonsai:'aesthetics',checked:null,works:{},agents:{},places:{},situations:{}};
const field={schema:FIELD_SCHEMA,spec:'bonsai/CORE.md',title:{ru:'Эстетика',en:'Aesthetics'},
  posture:{ru:'Предложение стенда: области с их вопросами, сквозные понятия и трактовки произведений. Читается стендом, экспортируется кандидатами, в ToS не записывается.',en:'The stand’s proposal: the areas with their questions, the crosscutting concepts and the works’ conceptions. Read by the stand, exported as candidates, never written into ToS.'},
  grown_from:{layer:'bonsai/review/aesthetics/layer/',readings:'bonsai/review/aesthetics/readings/',tool:'bonsai/review/aesthetics/plant.mjs'},
  makers:[
    {id:'registers-2026-11',date:'2026-11-07',made:['the areas and their questions','the concepts and their terms','the readings every conception grew from'],record:'bonsai/review/aesthetics/README.md'},
    {id:'claude-2026-11-09',model:'claude-fable-5-2',date:'2026-11-09',made:['the English titles of areas and concepts','the English questions of the areas'],record:'bonsai/review/aesthetics/inputs/'},
  ],
  readers:[...records.map(record=>({...record.reader,record:record.path??`bonsai/review/aesthetics/readings/${record.name}`})),...seconds],
  groups:['A','B','C','D'].map(id=>({id,areas:areasIn.filter(area=>area.group===id).map(area=>areaId[area.id])})),
  areas:areasIn.map(area=>({id:areaId[area.id],n:Number(area.id.slice(2)),group:area.group,type:'tos.entity.navigation-region',title:area.title,question:area.question,grew_from:area.id})),
  concepts:concepts.map(concept=>({id:conceptId[concept.key],type:concept.kinds.some(kind=>CATEGORY.test(kind))?'tos.entity.philosophical-category':'tos.entity.concept',
    title:{ru:capital(concept.title.ru),en:titles[concept.key]},terms:concept.terms,areas:concept.areas.map(area=>areaId[area]),grew_from:concept.keys})),
  conceptions:[],links:[]};

const layerWork=id=>works.find(work=>work.id===id)??fail(`no work ${id} in the layer`),layerAgent=id=>agents.find(agent=>agent.id===id)??fail(`no author ${id} in the layer`);
for(const record of records){
  for(const [id,entry] of Object.entries(record.library?.works??{})){
    const known=layerWork(id);
    library.works[id]={title:{...entry.title,...(known.title.original&&known.title.original!==entry.title.en?{original:known.title.original}:{})},agents:known.agents,original:entry.original,grew_from:known.keys};
    identities.works[id]={tree:entry.tree,status:existsSync(join(root,'ToS/source-witnesses/works',...treePath(entry.tree)))?'resolved':'to-mint'};
    for(const agent of known.agents){
      const person=layerAgent(agent);
      library.agents[agent]={name:person.name,grew_from:person.keys};
      const tree=record.library?.agents?.[agent]?.tree??`tos.agent.${agent}`;
      identities.agents[agent]={tree,status:existsSync(join(root,'ToS/source-witnesses/agents',tree.replace(/^tos\.agent\./,''),'agent.json'))?'resolved':'to-mint'};
    }
  }
  for(const conception of record.conceptions??[]){
    const libraryWork={...record.library.works[conception.work]};
    field.conceptions.push({id:conception.id,type:'tos.entity.conception',of:conception.of.map(key=>conceptId[key]??fail(`${conception.id}: unknown concept ${key}`)),
      areas:conception.areas.map(area=>areaId[area]),work:conception.work,agents:layerWork(conception.work).agents,
      ...(conception.voice?{voice:conception.voice}:{}),locus:conception.locus,
      formula:{cite:conception.formula.map(formula=>cite(record,conception,formula,libraryWork))},
      thesis:conception.thesis,...(conception.distinction?{distinction:conception.distinction}:{}),
      status:conception.status,reading:{reader:record.reader.id,verdict:conception.verdict,...(conception.note?{note:conception.note}:{})},
      admission:conception.verdict==='rejected'?'rejected':conception.verdict==='undecided'?'held':'map',grew_from:conception.from});
  }
  for(const link of record.links??[])field.links.push({id:link.id,from:conceptId[link.from_concept]??link.from_conception??fail(`${link.id}: no start`),to:conceptId[link.to_concept]??link.to_conception??fail(`${link.id}: no end`),verb:link.verb,
    ...(link.relation?{relation:link.relation}:{}),grounds:link.grounds,status:link.status,
    reading:{reader:record.reader.id,verdict:link.verdict,...(link.note?{note:link.note}:{})},admission:link.verdict==='rejected'?'rejected':link.verdict==='undecided'?'held':'map',grew_from:link.from});
}
/** Where the Tree keeps a work: retained Antiquity works under their author's folder, others under author/title. */
function treePath(tree){
  const [,,owner,title]=tree.split('.');
  if(owner!=='retained-antiquity')return [owner,title,'work.json'];
  const authors=readdirSync(join(root,'ToS/source-witnesses/works'),{withFileTypes:true}).filter(entry=>entry.isDirectory()).map(entry=>entry.name);
  const author=authors.find(name=>existsSync(join(root,'ToS/source-witnesses/works',name,title,'work.json')));
  return [author??owner,title,'work.json'];
}
const stored=existsSync(join(home,'identities.json'))?json(join(home,'identities.json')).checked:null;
identities.checked=stored??{tree_revision:null,date:null,basis:{ru:'Записи ToS/source-witnesses на этой ревизии: «resolved» — Древо уже держит эту идентичность, «to-mint» — её пришлось бы создать маршрутом владельца источника.',en:'ToS/source-witnesses records at this revision: resolved means the Tree already holds the identity; to-mint means its source-owner route would have to create it.'}};

// Participant lives are source data from the recorded Wikidata identities.
const lives=layer('lives'),lifeByItem=new Map(Object.values(lives).map(life=>[life.item,life]));
const participant=(person)=>{
  const item=person.qid??/^https:\/\/www\.wikidata\.org\/wiki\/(Q[1-9]\d*)\/?$/.exec(person.source??'')?.[1];
  const life=lifeByItem.get(item);
  return {role:person.role,name:person.name??life?.label??null,...(item?{qid:item}:{}),
    source:person.source??(item?`https://www.wikidata.org/wiki/${item}`:null),
    born:life?.born?.length?life.born:(Number.isInteger(person.born)?[{year:person.born}]:[]),died:life?.died?.length?life.died:(Number.isInteger(person.died)?[{year:person.died}]:[])};
};
for(const name of packets){
  const file=join(packetsDir,name,'evidence.json'),work=fragments.works[name];
  if(!work||!existsSync(file))continue;
  const people=json(file).persons??[],original=work.witnesses?.[work.original];
  if(original&&people.length)original.participants=people.map(participant);
}
const manualHands={
  'immanuel-kant.kritik-der-urteilskraft':{de:[['author','Q9312'],['editor','Q77045']],en:[['author','Q9312'],['translator','Q6221660']]},
  'edmund-burke.a-philosophical-enquiry-into-the-origin-of-our':{en:[['author','Q165792']]},
  'william-hogarth.the-analysis-of-beauty':{en:[['author','Q171344']]},
};
for(const [name,languages] of Object.entries(manualHands))for(const [lang,hands] of Object.entries(languages)){
  const witness=fragments.works[name]?.witnesses?.[lang];if(witness)witness.participants=hands.map(([role,qid])=>participant({role,qid}));
}
for(const work of Object.values(fragments.works))for(const witness of Object.values(work.witnesses??{})){
  if(witness.persons)witness.participants=witness.persons.map(participant);
}

// Upgrade legacy display rights without rewriting the signed reader packets.
// An omitted translation is not evidence of an unsuccessful search.
for(const work of Object.values(fragments.works)){
  for(const witness of Object.values(work.witnesses??{})){
    const rights=witness.rights??{};
    if(rights.basis&&['russia','usa','eu'].every(country=>typeof rights[country]==='string')){
      for(const country of ['russia','usa','eu'])rights[country]={basis:rights.basis,law:rights[country]};
      delete rights.basis;
    }
  }
  const notSearched=(work.absent??[]).filter(item=>!item.tried);
  if(notSearched.length){
    work.unsearched??=[];
    for(const item of notSearched)if(!work.unsearched.some(entry=>entry.language===item.language))work.unsearched.push(item);
    work.absent=(work.absent??[]).filter(item=>item.tried);
  }
}

const files={'field.json':field,'library.json':library,'identities.json':identities,'fragments.json':fragments};
if(process.argv.includes('--dry')){
  const problems=[...fieldIssues(field,{library}).map(issue=>`field: ${issue}`),...libraryIssues(library).map(issue=>`library: ${issue}`),...externalPlacesIssues(fragments).map(issue=>`places: ${issue}`)];
  for(const problem of problems)console.log(problem);
  console.log(`dry: ${field.conceptions.length} conceptions, ${field.links.length} links, ${Object.keys(fragments.places).length} works with places${tried?`; packet ${tried} read as accepted`:''}`);
  process.exit(problems.length?1:0);
}
const text=value=>`${JSON.stringify(value,null,1)}\n`;
if(process.argv.includes('--check')){
  let differs=false;
  for(const [name,value] of Object.entries(files)){const same=existsSync(join(home,name))&&readFileSync(join(home,name),'utf8')===text(value);console.log(`${name}: ${same?'matches':'differs from'} what the records give`);differs||=!same;}
  process.exit(differs?1:0);
}
for(const [name,value] of Object.entries(files))writeFileSync(join(home,name),text(value));
console.log(`aesthetics: ${field.areas.length} areas, ${field.concepts.length} concepts, ${field.conceptions.length} conceptions, ${field.links.length} links; ${Object.keys(library.works).length} works, ${Object.keys(library.agents).length} authors`);
