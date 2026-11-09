#!/usr/bin/env node
// Plants the Aesthetics bonsai from the research layer and the readers' records.
//
//   node bonsai/review/aesthetics/plant.mjs            # write bonsai/aesthetics/{field,library,identities}.json
//   node bonsai/review/aesthetics/plant.mjs --check    # say whether the stored modules are what the records give
//
// The areas and concepts come from layer/ and inputs/. A conception comes only from a record under readings/:
// a reader's account of one place, read in the work itself. The record names the formula by the words that open
// and close it in a unit of the public corpus; this tool finds the span, refuses one that does not run over whole
// sentences, and writes the citation. Fetch the corpus first: node bonsai/lib/layers.mjs fetch access/public-corpus

import {readFileSync,writeFileSync,readdirSync,existsSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {corpusReader} from '../../lib/corpus-align.mjs';
import {FIELD_SCHEMA,LIBRARY_SCHEMA} from '../../lib/field.mjs';

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
const chars=text=>Array.from(text);
// A sentence of the original ends on a full stop or a question mark; Greek asks with ";".
const ENDS=/[.;!?]/u;

/** The citation of a formula: the unit a record names by its reference and the span between the opening and closing words. */
function cite(record,conception,formula,libraryWork){
  const at=`${record.name} ${conception.id}`;
  const work=read?.(libraryWork.tree);
  if(!work)fail(`${at}: the corpus does not hold ${libraryWork.tree}; fetch it (node bonsai/lib/layers.mjs fetch access/public-corpus)`);
  const original=work.versions.find(version=>version.language===libraryWork.original);
  const units=original.units.filter(unit=>unit.reference===formula.unit);
  if(units.length!==1)fail(`${at}: ${units.length} units of the original are named ${formula.unit}`);
  const text=units[0].text,start=text.indexOf(formula.from),close=start<0?-1:text.indexOf(formula.to,start);
  if(start<0||close<0)fail(`${at}: the words of the formula are not in ${formula.unit}`);
  if(text.indexOf(formula.from,start+1)>=0&&text.indexOf(formula.from,start+1)<close)fail(`${at}: the opening words occur twice before the closing ones`);
  const end=close+formula.to.length,before=text.slice(0,start).trimEnd(),last=text.slice(0,end).trimEnd().slice(-1);
  if(before&&!ENDS.test(before.slice(-1))&&!/[A-ZΑ-Ω]$/u.test(before))fail(`${at}: the formula opens inside a sentence (${JSON.stringify(before.slice(-20))})`);
  if(!ENDS.test(last))fail(`${at}: the formula closes inside a sentence`);
  return {w:conception.work,u:[units[0].id,units[0].id],s:chars(text.slice(0,start)).length,e:chars(text.slice(0,end)).length};
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
  readers:records.map(record=>({...record.reader,record:`bonsai/review/aesthetics/readings/${record.name}`})),
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

const files={'field.json':field,'library.json':library,'identities.json':identities};
const text=value=>`${JSON.stringify(value,null,1)}\n`;
if(process.argv.includes('--check')){
  let differs=false;
  for(const [name,value] of Object.entries(files)){const same=existsSync(join(home,name))&&readFileSync(join(home,name),'utf8')===text(value);console.log(`${name}: ${same?'matches':'differs from'} what the records give`);differs||=!same;}
  process.exit(differs?1:0);
}
for(const [name,value] of Object.entries(files))writeFileSync(join(home,name),text(value));
console.log(`aesthetics: ${field.areas.length} areas, ${field.concepts.length} concepts, ${field.conceptions.length} conceptions, ${field.links.length} links; ${Object.keys(library.works).length} works, ${Object.keys(library.agents).length} authors`);
