#!/usr/bin/env node
// The chronology of the Aesthetics bonsai: the lives of its authors and the datings of its works.
//
//   node bonsai/review/aesthetics/chronology.mjs            write bonsai/aesthetics/chronology.json
//   node bonsai/review/aesthetics/chronology.mjs --fetch    first read from Wikidata the lives the layer does not hold yet
//   node bonsai/review/aesthetics/chronology.mjs --check    exit 1 when the module on disk differs from what the layer gives
//
// A life is read from Wikidata (layer/lives.json keeps what was fetched, with the item and the day). The dating of a
// work is the register's own: its wording, the year the layer reads in it, and the source the register names for the
// work. A work the register dates by a century stands inside its author's life and is no firmer than that life.

import {readFileSync,writeFileSync,readdirSync,existsSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chronologyIssues} from '../../lib/chronology.mjs';

const here=dirname(fileURLToPath(import.meta.url)),root=join(here,'..','..','..');
const read=path=>JSON.parse(readFileSync(path,'utf8'));
const layer=name=>read(join(here,'layer',`${name}.json`));
const LIVES=join(here,'layer','lives.json'),OUT=join(root,'bonsai','aesthetics','chronology.json');
const AGENT='ToS-bonsai/1 (+https://github.com/8Dionysus/Tree-of-Sophia)';
// The authors read by hand before the planting line: the line's packets name the Wikidata item of every other author.
const ITEMS={plato:'Q859',plotinus:'Q134189',aristotle:'Q868','immanuel-kant':'Q9312','edmund-burke':'Q165792','william-hogarth':'Q171344'};

const library=read(join(root,'bonsai','aesthetics','library.json'));
const packets=join(here,'planting','packets');
for(const name of existsSync(packets)?readdirSync(packets):[]){
  const file=join(packets,name,'evidence.json');if(!existsSync(file))continue;
  const item=(read(file).persons??[]).filter(person=>person.role==='author')
    .map(person=>person.qid??/^https:\/\/www\.wikidata\.org\/wiki\/(Q[1-9]\d*)\/?$/.exec(person.source??'')?.[1]).find(Boolean);
  const agent=(library.works[name]?.agents??[])[0];
  if(item&&agent&&!ITEMS[agent])ITEMS[agent]=item;
}

const args=new Set(process.argv.slice(2));
const lives=existsSync(LIVES)?read(LIVES):{};
if(args.has('--fetch')){
  for(const agent of Object.keys(library.agents)){
    if(lives[agent])continue;
    const item=ITEMS[agent];if(!item)throw new Error(`${agent}: no Wikidata item is named for this author`);
    const answer=await fetch(`https://www.wikidata.org/wiki/Special:EntityData/${item}.json`,{headers:{'User-Agent':AGENT}});
    if(!answer.ok)throw new Error(`${agent}: Wikidata answers ${answer.status} for ${item}`);
    const entity=(await answer.json()).entities[item];
    const times=property=>{
      const claims=(entity.claims[property]??[]).filter(claim=>claim.rank!=='deprecated'&&claim.mainsnak.datavalue),preferred=claims.filter(claim=>claim.rank==='preferred');
      return (preferred.length?preferred:claims).map(claim=>({time:claim.mainsnak.datavalue.value.time,precision:claim.mainsnak.datavalue.value.precision}));
    };
    lives[agent]={item,label:{en:entity.labels.en?.value??null,ru:entity.labels.ru?.value??null},born:times('P569'),died:times('P570'),fetched:new Date().toISOString().slice(0,10)};
    await new Promise(resolve=>setTimeout(resolve,400));
  }
  writeFileSync(LIVES,`${JSON.stringify(Object.fromEntries(Object.keys(lives).sort().map(key=>[key,lives[key]])),null,1)}\n`);
}

// Wikidata writes a year before the era without a year zero; the Tree counts astronomically (1 BCE = 0).
const yearOf=time=>{const year=Number(/^([+-]\d+)-/.exec(time)[1]);return year<0?year+1:year;};
const lexeme=year=>`${year<0?'-':''}${String(Math.abs(year)).padStart(4,'0')}`;
const spoken=(years,locale)=>{                                        // 428/427 BCE, 1632
  const shown=[...new Set(years)].map(year=>year<=0?1-year:year).join('/'),before=years.some(year=>year<=0);
  return before?`${shown} ${locale==='ru'?'до н. э.':'BCE'}`:shown;
};
const between=(from,to,locale)=>{
  const before=from.some(year=>year<=0)&&to.some(year=>year<=0),bare=years=>[...new Set(years)].map(year=>year<=0?1-year:year).join('/');
  return before?`${bare(from)}–${bare(to)} ${locale==='ru'?'до н. э.':'BCE'}`:`${spoken(from,locale)}–${spoken(to,locale)}`;
};
const dating=(more)=>({role:'historical-time',calendar:null,year_numbering:'astronomical',...more});

const sources={},situations={},lifeOf={},workOf={};
for(const [agent,entry] of Object.entries(library.agents)){
  const life=lives[agent];if(!life)throw new Error(`${agent}: no life in layer/lives.json (run with --fetch)`);
  const born=life.born.map(item=>yearOf(item.time)),died=life.died.map(item=>yearOf(item.time));
  if(!born.length||!died.length)throw new Error(`${agent}: Wikidata ${life.item} gives no year of birth or of death`);
  const exact=new Set(born).size===1&&new Set(died).size===1&&[...life.born,...life.died].every(item=>item.precision>=9);
  const id=`tos.biographical-phase.${agent}.lifetime`,source=`wikidata-${agent}`;
  sources[source]={short:'Wikidata',title:`${life.label.en??entry.name.en} (${life.item})`,url:`https://www.wikidata.org/wiki/${life.item}`};
  situations[id]={type:'biographical-phase',subject:agent,label:{ru:`${entry.name.ru}: годы жизни`,en:`${entry.name.en}: lifetime`},
    dating:{kind:'interval-assertion',...dating({certainty:exact?'exact':'approximate',interval:{start:lexeme(Math.min(...born)),end:lexeme(Math.max(...died))},source_wording:{text:between(born,died,'en'),language:'en'}})},
    statement:{ru:`Викиданные: ${entry.name.ru}, ${between(born,died,'ru')}.`,en:`Wikidata: ${entry.name.en}, ${between(born,died,'en')}.`},
    sources:[source],footing:'medium',epistemic:'reported'};
  lifeOf[agent]=id;
}

const works=new Map(layer('works').map(work=>[work.id,work])),named=new Map(layer('sources').map(source=>[source.key,source]));
for(const [work,entry] of Object.entries(library.works)){
  const record=works.get(work);if(!record)throw new Error(`${work}: the layer holds no such work`);
  const cited=(record.sources??[]).map(key=>named.get(key)).find(source=>/^https:\/\//.test(source?.url??''));
  if(!cited)throw new Error(`${work}: the register names no source with an address`);
  const wording=(record.dates??[])[0]??'',agent=entry.agents[0],source=`register-${cited.key.replace(/[^A-Za-z0-9]+/g,'-').toLowerCase()}`;
  sources[source]={short:cited.title.split(/[.:—]/)[0].trim().slice(0,40),title:cited.title,url:cited.url};
  // The register's first span of years is the work's own when it opens with the year the layer reads; a later span names an edition or a translation.
  const year=record.year,found=/(\d{4})\s*[–-]\s*(\d{4})/.exec(wording),span=found&&Number(found[1])===year?found:null;
  const when=span?{kind:'interval-assertion',...dating({certainty:wording.trim()===span[0]?'exact':'approximate',interval:{start:span[1],end:span[2]},source_wording:{text:wording,language:'ru'}})}
    :Number.isInteger(year)?{kind:'date-assertion',...dating({certainty:wording.trim()===String(year)?'exact':'approximate',value:lexeme(year),source_wording:{text:wording,language:'ru'}})}
    :{kind:'relative-order',role:'historical-time',calendar:null,year_numbering:null,certainty:'approximate',relative:{relation:'during',anchor_ref:lifeOf[agent]},source_wording:{text:wording,language:'ru'}};
  const title={ru:entry.title.ru,en:entry.title.en??entry.title.original??entry.title.ru};
  const said=span?`${span[1]}–${span[2]}`:Number.isInteger(year)?String(year):`within the life of ${library.agents[agent].name.en}`;
  const id=`tos.historical-process.${work}-dating`;
  situations[id]={type:'historical-process',work,label:{ru:`Датировка: «${title.ru}»`,en:`Dating: ${title.en}`},dating:when,
    statement:{ru:`Реестр бонсая датирует произведение: ${wording}. Источник реестра: ${cited.title}.`,en:`The bonsai’s register dates the work ${said}. The register’s source: ${cited.title}.`},
    sources:[source],footing:'medium',epistemic:when.kind==='relative-order'?'inferred':'reported'};
  workOf[work]=id;
}

const checked=Object.values(lives).map(life=>life.fetched).sort().at(-1);
const module={schema:'tos.bonsai.chronology.v1',bonsai:'aesthetics',checked,
  posture:{ru:'Годы жизни авторов и датировки произведений в грамматике дат Древа. Жизнь прочитана в Викиданных; датировка произведения — слова реестра бонсая и источник, который реестр называет. Это предложение стенда.',
    en:'The lives of the authors and the datings of the works in the Tree’s date grammar. A life is read from Wikidata; the dating of a work is the wording of the bonsai’s register and the source the register names. It is the stand’s proposal.'},
  numbering:{year_numbering:'astronomical',calendar:null,note:{ru:'Годы до н. э. переведены в астрономический счёт (1 г. до н. э. = 0); календарь источники не называют, и он не утверждается.',en:'Years BCE are converted to astronomical numbering (1 BCE = 0); the sources name no calendar and none is asserted.'}},
  sources,lives:lifeOf,compositions:{works:workOf,passages:{}},situations};
const issues=chronologyIssues(module);
if(issues.length){console.error(issues.map(issue=>`chronology: ${issue}`).join('\n'));process.exit(1);}
const text=`${JSON.stringify(module,null,1)}\n`;
if(args.has('--check')){
  const same=existsSync(OUT)&&readFileSync(OUT,'utf8')===text;
  console.log(same?'chronology: as the layer gives it':'chronology: the module on disk differs from what the layer gives');process.exit(same?0:1);
}
writeFileSync(OUT,text);
console.log(`chronology: ${Object.keys(lifeOf).length} lives, ${Object.keys(workOf).length} works`);
