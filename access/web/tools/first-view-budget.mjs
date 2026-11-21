#!/usr/bin/env node
// What each public page carries before its first view: the scripts and stylesheets its HTML names, with the
// modules Vite preloads for them. Anything a page fetches later (a language, the retained library, a stand's
// layers) is not counted. The limits are data (tools/first-view-budget.json), set from a measured build.
//
//   node tools/first-view-budget.mjs <dist> [budget.json]
import {existsSync,readFileSync,statSync} from 'node:fs';
import {dirname,join,relative,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

const here=dirname(fileURLToPath(import.meta.url));
const [distArg,budgetArg]=process.argv.slice(2);
if(!distArg){console.error('usage: first-view-budget.mjs <dist> [budget.json]');process.exit(2);}
const dist=resolve(distArg),budget=JSON.parse(readFileSync(budgetArg??join(here,'first-view-budget.json'),'utf8'));
if(budget.schema!=='tos_web_first_view_budget_v1')throw new Error('Unsupported first-view budget schema.');

// An asset address names the build's base first (/static/, /__research/); the file lies under the rest.
function fileOf(address){
  const parts=new URL(address,'http://site/').pathname.split('/').filter(Boolean);
  for(let i=0;i<parts.length;i++){const candidate=join(dist,...parts.slice(i));if(existsSync(candidate)&&statSync(candidate).isFile())return candidate;}
  return null;
}
function firstView(page){
  const html=readFileSync(join(dist,page),'utf8'),files=new Set(),missing=[];
  for(const tag of html.match(/<(?:script|link)\b[^>]*>/gi)??[]){
    const rel=/\brel=["']?([^"'\s>]+)/i.exec(tag)?.[1]?.toLowerCase();
    const address=/^<script/i.test(tag)?/\bsrc=["']([^"']+)["']/i.exec(tag)?.[1]:['modulepreload','stylesheet'].includes(rel)?/\bhref=["']([^"']+)["']/i.exec(tag)?.[1]:null;
    if(!address||/^(?:[a-z]+:)?\/\//i.test(address))continue;
    const file=fileOf(address);if(file)files.add(file);else missing.push(address);
  }
  return {files:[...files],bytes:[...files].reduce((sum,file)=>sum+statSync(file).size,0),missing};
}

let failed=false;
for(const [page,limit] of Object.entries(budget.pages)){
  if(!existsSync(join(dist,page))){console.error(`${page}: absent from ${dist}`);failed=true;continue;}
  const view=firstView(page),over=view.bytes>limit.max_bytes;
  const largest=view.files.map(file=>[statSync(file).size,relative(dist,file)]).sort((a,b)=>b[0]-a[0])[0];
  console.log(`${over?'over':'within'} ${page}: ${view.bytes} of ${limit.max_bytes} bytes in ${view.files.length} files${largest?`; largest ${largest[1]} ${largest[0]}`:''}`);
  if(view.missing.length){console.error(`${page}: names files the build lacks: ${view.missing.join(', ')}`);failed=true;}
  if(over)failed=true;
}
process.exit(failed?1:0);
