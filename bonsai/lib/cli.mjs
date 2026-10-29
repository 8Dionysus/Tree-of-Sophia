#!/usr/bin/env node
// Check a bonsai against the Tree, or write its export packet.
//
//   node bonsai/lib/cli.mjs check [<bonsai>...]
//   node bonsai/lib/cli.mjs export <bonsai> [--out <file>]
//
// `check` exits non-zero when a module breaks its contract or the export does
// not satisfy the Tree's contracts in this checkout; drift since the graft is
// reported, not failed. `export` writes the packet as JSON (stdout by default);
// it never writes into the Tree.

import {writeFileSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {bonsaiIds,checkBonsai} from './graft.mjs';

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
}else{
  console.error('usage: node bonsai/lib/cli.mjs check [<bonsai>...] | export <bonsai> [--out <file>]');
  process.exit(2);
}
