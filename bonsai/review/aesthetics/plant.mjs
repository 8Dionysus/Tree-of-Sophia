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
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {corpusReader} from '../../lib/corpus-align.mjs';
import {FIELD_SCHEMA,LIBRARY_SCHEMA,fieldIssues,libraryIssues} from '../../lib/field.mjs';
import {externalPlacesIssues} from '../../lib/fragments.mjs';
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
  .filter(name=>existsSync(join(packetsDir,name,'accepted.json'))||name===tried);
const seconds=[];
for(const name of packets){
  const at=join(packetsDir,name),path=`bonsai/review/aesthetics/planting/packets/${name}`,places=json(join(at,'places.json'));
  records.push({name:`planting/packets/${name}/reading.json`,path:`${path}/reading.json`,...json(join(at,'reading.json'))});
  if(existsSync(join(at,'review.json'))){const {reader}=json(join(at,'review.json'));seconds.push({...reader,record:`${path}/review.json`});}
  if(places.entry)fragments.works[name]=places.entry;
  if(!fragments.works[name])fail(`packet ${name}: no entry for a work the places module does not hold`);
  fragments.places[name]={...(places.entry?{}:fragments.places[name]),...places.places};
  const own=new Set(Object.keys(places.places).map(loc=>`${name} ${loc}`)),seen=fragments.provenance.checked_on_the_page_image;
  fragments.provenance.corrections=[...fragments.provenance.corrections.filter(correction=>!own.has(correction.place)),...(places.corrections??[])];
  if(places.checked_on_the_page_image?.length)seen[name]=[...new Set([...(places.entry?[]:seen[name]??[]),...places.checked_on_the_page_image])];
}

/** The citation of a formula: the unit a record names by its reference and the span between the opening and closing words. */
function cite(record,conception,formula,libraryWork){
  const at=`${record.name} ${conception.id}`;
  if(libraryWork.places==='fragments'){
    const text=fragments?.places?.[conception.work]?.[formula.place]?.texts?.[libraryWork.original];
    if(fragments?.works?.[conception.work]?.original!==libraryWork.original)fail(`${at}: fragments.json does not hold ${conception.work} in ${libraryWork.original}`);
    if(typeof text!=='string')fail(`${at}: fragments.json holds no place ${JSON.stringify(formula.place)} of ${conception.work} in the original`);
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
