#!/usr/bin/env node
// The pages of a witness's edition (TOS-D-0084): for each unit of a witness, the printed page or pages it stands on,
// with the holder's address of each page image. A page map is read from the holder's own record of the edition: the
// Deutsches Textarchiv's TEI, whose <pb facs=… n=…/> marks each page break, or the transcribed pages of a scan on
// Wikisource. The units are found in that record by their letters alone (spaces, line ends, broken words, the old
// orthography's letters and hard signs aside), in order, so a unit's page follows from where its text stands.
// Nothing here changes a witness; the map is stand data kept beside it.
import {readFileSync,writeFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {dirname,join} from 'node:path';
import {readChapter} from './witnesses.mjs';
import {createHash} from 'node:crypto';

export const PAGES_SCHEMA='tos.bonsai.pages.v1';
const OLD={'ѣ':'е','і':'и','ѳ':'ф','ѵ':'и','ё':'е'};

/** A text's letters alone, in lower case, the old Russian letters as today's, hard signs left out. */
export function skeleton(text){
  let out='';
  for(const char of String(text??'').normalize('NFC').toLowerCase()){
    const letter=OLD[char]??char;
    if(letter==='ъ')continue;
    if(/[\p{L}\p{N}]/u.test(letter))out+=letter;
  }
  return out;
}

const decode=text=>text.replace(/&(amp|lt|gt|quot|apos|#\d+|#x[0-9a-f]+);/gi,(match,name)=>({amp:'&',lt:'<',gt:'>',quot:'"',apos:'\''})[name.toLowerCase()]??(name[1]?.toLowerCase()==='x'?String.fromCodePoint(parseInt(name.slice(2),16)):String.fromCodePoint(Number(name.slice(1)))));
const attribute=(tag,name)=>new RegExp(`\\s${name}="([^"]*)"`).exec(tag)?.[1]??null;

/**
 * A TEI record's body as one stream of letters and its page breaks: each break with its offset in the stream, the
 * printed page number (`n`) and the facsimile it names (`facs`). Running heads, catchwords and notes are not the text.
 */
export function teiStream(xml){
  const body=String(xml).split(/<body[\s>]/)[1]?.split('</body>')[0]??String(xml);
  let stream='',skip=0;const breaks=[];
  for(const part of body.split(/(<[^>]+>)/)){
    if(!part)continue;
    if(part.startsWith('<')){
      const closing=part.startsWith('</'),name=/^<\/?([\w:]+)/.exec(part)?.[1]??'',empty=part.endsWith('/>');
      if(['fw','note','teiHeader','sic'].includes(name)){if(!empty)skip+=closing?-1:1;continue;}
      if(name==='pb'&&!skip)breaks.push({offset:stream.length,n:attribute(part,'n'),facs:attribute(part,'facs')?.replace(/^#/,'')??null});
      continue;
    }
    if(!skip)stream+=skeleton(decode(part));
  }
  return {stream,breaks};
}

/** Transcribed scan pages ({scan, text}) as one stream with a break at the start of each page. */
export function pagesStream(pages){
  let stream='';const breaks=[];
  for(const page of [...pages].sort((a,b)=>a.scan-b.scan)){
    const text=String(page.text??'').replace(/<noinclude>[\s\S]*?<\/noinclude>/g,'').replace(/\{\{[^{}]*\}\}/g,' ').replace(/<[^>]+>/g,' ').replace(/\[\[(?:[^|\]]*\|)?([^\]]*)\]\]/g,'$1').replace(/'{2,}/g,'');
    breaks.push({offset:stream.length,n:page.n??String(page.scan),scan:page.scan});
    stream+=skeleton(text);
  }
  return {stream,breaks};
}

/**
 * Each unit's pages: the unit is looked for after the previous one, by its first letters (shorter anchors where the
 * longer one is not found), and its pages are the break before its start through the break before its end.
 * Returns {units: {id: [first, last]}, missing: [id]} with page indexes into `breaks`.
 */
export function unitPages(units,{stream,breaks},{window=20000}={}){
  const pageAt=offset=>{let low=0,high=breaks.length-1,found=-1;while(low<=high){const mid=(low+high)>>1;if(breaks[mid].offset<=offset){found=mid;low=mid+1;}else high=mid-1;}return found;};
  const out={},missing=[];let cursor=0;
  for(const unit of units){
    const letters=skeleton(unit.raw);if(!letters){missing.push(unit.id);continue;}
    let at=-1,used=0;
    for(const size of [48,32,20,12]){
      const anchor=letters.slice(0,size);if(anchor.length<Math.min(size,letters.length))continue;
      const found=stream.indexOf(anchor,cursor);
      if(found>=0&&found-cursor<=window){at=found;used=anchor.length;break;}
    }
    if(at<0){missing.push(unit.id);continue;}
    const end=at+Math.max(used,letters.length)-1,first=pageAt(at),last=pageAt(Math.min(stream.length-1,end));
    if(first>=0)out[unit.id]=[first,Math.max(first,last)];
    cursor=at+Math.max(1,Math.floor(letters.length*.9));
  }
  return {units:out,missing};
}

/** Scan pages from a list of {scan, n?, text} or from MediaWiki API answers (query.pages with each page's wikitext). */
export function scanPages(value){
  if(Array.isArray(value))return value;
  return Object.values(value?.query?.pages??{}).map(page=>({scan:Number(/\/(\d+)$/.exec(page.title??'')?.[1]),text:page.revisions?.[0]?.slots?.main?.['*']??page.revisions?.[0]?.['*']??''})).filter(page=>Number.isInteger(page.scan)&&page.text);
}

/** A page map for a witness from its record: pages with their labels and images, units with their page spans. */
export function pageMap({witness,source,image,rights,units,record}){
  const {units:spans,missing}=unitPages(units,record);
  return {schema:PAGES_SCHEMA,witness,source,rights,
    pages:record.breaks.map(page=>({n:page.n??null,...(page.facs?{facs:page.facs}:{}),...(page.book?{book:page.book}:{}),...(page.scan?{scan:page.scan}:{}),image:image(page)})),
    units:spans,missing};
}

/** A DTA book's page images: the holder's own copy of each facsimile at 800 pixels. */
export const dtaImage=fallback=>page=>{const book=page.book??fallback;return page.facs&&book?`https://media.dwds.de/dta/images/${book}/${book}_${page.facs.replace(/^f/,'').padStart(4,'0')}_800px.jpg`:null;};
/** A Commons PDF scan's page as the holder renders it. */
export const commonsPdfPage=file=>page=>`https://commons.wikimedia.org/w/thumb.php?f=${encodeURIComponent(file)}&page=${page.scan}&w=800`;

const sha256=bytes=>createHash('sha256').update(bytes).digest('hex');

/** A witness's units in reading order, from the stand's chapter files: the list the builder looks for. */
export function witnessUnits(standDir,language){
  const manifest=JSON.parse(readFileSync(join(standDir,'witnesses.json'),'utf8'));
  return manifest.chapters.flatMap(chapter=>readChapter(JSON.parse(readFileSync(join(standDir,'witnesses',`${chapter.key}.json`),'utf8'))).units[language]??[]).map(unit=>({id:unit.id,raw:unit.raw}));
}

function runCli(args){
  if(args[0]==='units'){
    const [,language,standDir=join(dirname(fileURLToPath(import.meta.url)),'..','zarathustra')]=args;
    if(!language){console.error('Usage: node bonsai/lib/pages.mjs units LANGUAGE [STAND_DIR] > UNITS.json');process.exitCode=2;return;}
    process.stdout.write(`${JSON.stringify({witness:language,units:witnessUnits(standDir,language),books:[],file:null,rights:{statement:null,url:null}})}\n`);return;
  }
  const [kind,unitsFile,output,...inputs]=args;
  if(!['dta','wikisource'].includes(kind)||!unitsFile||!output||!inputs.length){
    console.error('Usage: node bonsai/lib/pages.mjs dta|wikisource UNITS.json OUTPUT.json SOURCE...\n  UNITS.json (from `pages.mjs units LANGUAGE`): {witness, units:[{id, raw}], books (DTA, one per SOURCE), file (Commons PDF), rights:{statement, url}}');
    process.exitCode=2;return;
  }
  const spec=JSON.parse(readFileSync(unitsFile,'utf8'));
  const bytes=inputs.map(path=>readFileSync(path));
  const record=kind==='dta'
    // The parts of a book come as files in reading order; `books` names each file's DTA book for its page images.
    ?bytes.map((value,i)=>({...teiStream(value.toString('utf8')),book:spec.books?.[i]??null})).reduce((all,part)=>({stream:all.stream+part.stream,breaks:[...all.breaks,...part.breaks.map(page=>({...page,book:part.book,offset:page.offset+all.stream.length}))]}),{stream:'',breaks:[]})
    :pagesStream(bytes.flatMap(value=>scanPages(JSON.parse(value.toString('utf8')))));
  const map=pageMap({witness:spec.witness,units:spec.units,record,rights:{...spec.rights,checked:false},
    source:{kind,inputs:inputs.map((path,i)=>({name:path.split('/').at(-1),sha256:sha256(bytes[i])}))},
    image:kind==='dta'?dtaImage(spec.books?.[0]??null):commonsPdfPage(spec.file)});
  writeFileSync(output,`${JSON.stringify(map)}\n`);
  console.log(`${spec.witness}: ${Object.keys(map.units).length} units on ${map.pages.length} pages; ${map.missing.length} not found`);
}
if(process.argv[1]&&import.meta.url===new URL(`file://${process.argv[1]}`).href)runCli(process.argv.slice(2));
