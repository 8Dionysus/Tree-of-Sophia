#!/usr/bin/env node
// Check a bonsai against the Tree, or write its export packet.
//
//   node bonsai/lib/cli.mjs check [<bonsai>...]
//   node bonsai/lib/cli.mjs export <bonsai> [--out <file>]
//   node bonsai/lib/cli.mjs align <bonsai> [--write]
//   node bonsai/lib/cli.mjs align-corpus <bonsai> [--write] [--corpus <dir>]
//
// `check` exits non-zero when a module breaks its contract or the export does
// not satisfy the Tree's contracts in this checkout; drift since the graft is
// reported, not failed. `export` writes the packet as JSON (stdout by default);
// it never writes into the Tree. `align` aligns the bonsai's witnesses again
// from their stored texts with the mechanism in align.mjs and reports where the
// stored rows differ; `--write` stores the new rows and receipt instead.
// `align-corpus` aligns, with corpus-align.mjs, the public corpus works the
// stand's content cites (its own modules and the modules it borrows) and
// reports whether the stored alignment module still matches; `--write` stores it.

import {readFileSync,writeFileSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {alignCorpus,citedUnits,corpusReader,formatAlignment} from './corpus-align.mjs';
import {bonsaiIds,checkBonsai,loadBonsai,loadWitnessChapters} from './graft.mjs';
import {realignWitnesses} from './realign.mjs';

const root=join(dirname(fileURLToPath(import.meta.url)),'..','..');
const [command,...rest]=process.argv.slice(2);
const option=name=>{const index=rest.indexOf(name);return index>=0?rest[index+1]:null;};
const names=rest.filter((value,index)=>!value.startsWith('--')&&!(index>0&&rest[index-1].startsWith('--')));

if(command==='check'){
  let failed=false;
  for(const id of names.length?names:bonsaiIds(root)){
    const result=checkBonsai(root,id);
    const counts=result.packet?`${result.packet.claims.length} claims, ${result.packet.records.length} records, ${result.packet.identities.length} identities`:'no export';
    console.log(`${id}: ${result.problems.length?'FAILED':'ok'} · ${counts}${result.checkedIdentities?'':' · Tree identities not checked out'}`);
    for(const problem of result.problems)console.log(`  problem: ${problem}`);
    for(const item of result.drift)console.log(`  drift: ${item}`);
    failed||=result.problems.length>0;
  }
  process.exit(failed?1:0);
}else if(command==='export'&&names.length===1){
  const result=checkBonsai(root,names[0]);
  if(result.problems.length){for(const problem of result.problems)console.error(`problem: ${problem}`);process.exit(1);}
  const text=`${JSON.stringify(result.packet,null,2)}\n`,out=option('--out');
  if(out)writeFileSync(out,text);else process.stdout.write(text);
}else if(command==='align'&&names.length===1){
  const {home,manifest,modules}=loadBonsai(root,names[0]);
  if(!modules.witnesses){console.error(`${names[0]} has no witnesses module`);process.exit(2);}
  const chapters=loadWitnessChapters(home,manifest),result=realignWitnesses(modules.witnesses,chapters);
  console.log(JSON.stringify(result.receipt));
  console.log(`${result.changed.length} chapters whose rows differ from the stored alignment${result.changed.length?`: ${result.changed.join(' ')}`:''}`);
  if(rest.includes('--write')){
    const module=manifest.modules.find(item=>item.id==='witnesses'),dir=join(home,dirname(module.chapters));
    for(const [key,file] of Object.entries(result.chapters))writeFileSync(join(dir,`${key}.json`),`${JSON.stringify(file)}\n`);
    writeFileSync(join(home,module.path),`${JSON.stringify({...modules.witnesses,alignment:{...modules.witnesses.alignment,...result.alignment}},null,2)}\n`);
    console.log('stored');
  }else if(result.changed.length)process.exit(1);
}else if(command==='align-corpus'&&names.length===1){
  const {home,manifest,modules}=loadBonsai(root,names[0]),module=(manifest.modules??[]).find(item=>item.id==='alignment');
  if(!module){console.error(`${names[0]} has no alignment module`);process.exit(2);}
  // What the stand cites: its own modules and the exports of the modules it borrows.
  const content=[];
  for(const item of manifest.modules??[]){
    if(item.borrowed){for(const path of [].concat(item.borrowed))if(path.endsWith('.mjs'))content.push(Object.values(await import(pathToFileURL(join(root,path)).href)));}
    else if(item.id!=='alignment'&&modules[item.id])content.push(modules[item.id]);
  }
  const works=Object.fromEntries(Object.entries(modules.identities?.works??{}).map(([key,value])=>[key,value.tree]));
  const read=corpusReader(join(root,option('--corpus')??'access/web/public/public-corpus'));
  const text=formatAlignment(alignCorpus(citedUnits(content,works,read),works,read)),path=join(home,module.path);
  const same=readFileSync(path,'utf8')===text;
  console.log(`${names[0]}: the stored corpus alignment ${same?'matches':'differs from'} the one the mechanism gives`);
  if(rest.includes('--write')){writeFileSync(path,text);console.log('stored');}
  else if(!same)process.exit(1);
}else{
  console.error('usage: node bonsai/lib/cli.mjs check [<bonsai>...] | export <bonsai> [--out <file>] | align <bonsai> [--write] | align-corpus <bonsai> [--write] [--corpus <dir>]');
  process.exit(2);
}
