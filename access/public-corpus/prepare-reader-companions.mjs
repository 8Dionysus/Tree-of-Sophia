#!/usr/bin/env node
// Copy the two bounded reader companions from their authored sources.
import {readFileSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';

export const names=['reader-textual-variants.v1.json','contained-reading-targets.v1.json'];
export function prepareReaderCompanions(root,{write=false}={}){
  const source=join(root,'ToS/research-packets/public-prototypes-20261026');
  const target=join(root,'access/public-corpus');
  for(const name of names){
    const content=readFileSync(join(source,name));
    if(content.length>64_000)throw new Error(`reader companion exceeds its bounded metadata budget: ${name}`);
    const destination=join(target,name);
    if(write)writeFileSync(destination,content);
    else {
      let actual;
      try{actual=readFileSync(destination);}catch(error){if(error.code!=='ENOENT')throw error;}
      if(!actual?.equals(content))throw new Error(`reader companion differs from its source: ${name}`);
    }
  }
  return names.length;
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href){
  const args=process.argv.slice(2);
  if(args.some(arg=>arg!=='--write')||args.length>1){console.error('usage: node prepare-reader-companions.mjs [--write]');process.exit(2);}
  try{console.log(`Reader companions match their authored sources: ${prepareReaderCompanions(resolve(dirname(fileURLToPath(import.meta.url)),'../..'),{write:args.includes('--write')})}`);}
  catch(error){console.error(error.message);process.exit(1);}
}
