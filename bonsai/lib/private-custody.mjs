// Private source custody: no public mirror, transfer branch or upload route.
import {createHash} from 'node:crypto';
import {chmodSync, linkSync, lstatSync, mkdirSync, readFileSync, unlinkSync, writeFileSync} from 'node:fs';
import {dirname, isAbsolute, join, resolve, sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import {signS3} from './layers.mjs';

const source='ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra';
const manifests=[
  'ToS/source-witnesses/artifacts/private-custody.v1.json',
  `${source}/technical-markup/antonovsky-1911-pdf-layout-v1/r2-manifest.v1.json`,
  `${source}/technical-markup/antonovsky-1911-structural-paragraph-v2/r2-manifest.v1.json`,
  `${source}/technical-markup/dta-first-editions-parts-1-4-v1/r2-manifest.v1.json`,
  `${source}/lexical-indexes/dta-antonovsky-parallel-candidates-v1/r2-manifest.v1.json`,
  `${source}/alignments/translation/dta-first-editions-to-antonovsky-1911-paragraph-v1/r2-manifest.v1.json`,
];
const digest=bytes=>createHash('sha256').update(bytes).digest('hex');
const safePath=path=>typeof path==='string' && path.startsWith('ToS/source-witnesses/') && !path.split('/').some(p=>!p || p==='.' || p==='..') && !path.includes('\\');
const validRevision=value=>typeof value==='string' && /^[0-9a-f]{40}$/.test(value);

export function inventory(repo){
  const entries=[], seen=new Set();
  for(const manifest of manifests){
    const data=JSON.parse(readFileSync(join(repo,manifest),'utf8'));
    if(!['tos_private_custody_v1','tos_r2_manifest_v1'].includes(data.schema_version) || !Array.isArray(data.entries) || !data.entries.length) throw new Error(`invalid custody manifest: ${manifest}`);
    const family=manifest.endsWith('/private-custody.v1.json')?'ToS/source-witnesses/artifacts/':`${dirname(manifest)}/`;
    for(const e of data.entries){
      if(!safePath(e.path) || !e.path.startsWith(family) || seen.has(e.path) || !/^[0-9a-f]{64}$/.test(e.sha256) || !Number.isSafeInteger(e.bytes) || e.bytes<0 || e.bucket!=='tos-source-bytes' || e.access!=='private' || e.key!==`sha256/${e.sha256}` || !validRevision(e.source_revision)) throw new Error(`invalid custody entry: ${e.path}`);
      seen.add(e.path); entries.push(e);
    }
  }
  return entries;
}

function safeDestination(root, path, create){
  const base=resolve(root), target=resolve(base,path);
  if(!target.startsWith(base+sep)) throw new Error(`unsafe path: ${path}`);
  const rootInfo=lstatSync(base);
  if(rootInfo.isSymbolicLink() || !rootInfo.isDirectory()) throw new Error(`unsafe root: ${base}`);
  let current=base;
  for(const part of path.split('/').slice(0,-1)){
    current=join(current,part);
    try {const s=lstatSync(current); if(s.isSymbolicLink() || !s.isDirectory()) throw new Error(`unsafe directory: ${current}`);}
    catch(error){if(error.code!=='ENOENT') throw error; if(create) mkdirSync(current,{mode:0o700}); else return target;}
  }
  return target;
}

function existing(path){
  try {const s=lstatSync(path); if(!s.isFile() || s.isSymbolicLink()) throw new Error(`not a regular file: ${path}`); return readFileSync(path);}
  catch(error){if(error.code==='ENOENT') return null; throw error;}
}
function verify(entry,bytes){if(bytes.length!==entry.bytes || digest(bytes)!==entry.sha256) throw new Error(`private custody size or SHA-256 mismatch: ${entry.path}`);}

function privateR2(env){
  const {R2_ENDPOINT:endpoint,R2_ACCESS_KEY_ID:accessKeyId,R2_SECRET_ACCESS_KEY:secretAccessKey}=env;
  if(!endpoint || !accessKeyId || !secretAccessKey) return null;
  const base=new URL(endpoint);
  if(base.protocol!=='https:' || base.username || base.password || base.search || base.hash) throw new Error('R2_ENDPOINT must be an HTTPS endpoint without credentials, query or fragment');
  return async entry=>{
    const url=`${base.href.replace(/\/+$/,'')}/tos-source-bytes/${entry.key}`;
    const headers=signS3({method:'GET',url,payloadHash:digest(Buffer.alloc(0)),accessKeyId,secretAccessKey});
    delete headers.host;
    const response=await fetch(url,{headers,redirect:'error',signal:AbortSignal.timeout(120000)});
    if(!response.ok) throw new Error(`private R2 GET ${entry.key}: ${response.status}`);
    const chunks=[]; let size=0;
    for await(const chunk of response.body){size+=chunk.length; if(size>entry.bytes) throw new Error(`private custody object exceeds declared size: ${entry.path}`); chunks.push(chunk);}
    return Buffer.concat(chunks,size);
  };
}

export async function hydrate(repo,{target=repo,objects=null,recovery=null,check=false,log=console.log}={}){
  const entries=inventory(repo);
  if(objects && recovery) throw new Error('choose either --objects or --recovery');
  const r2=!check && !objects && !recovery?privateR2(process.env):null;
  if(!check && !objects && !recovery && !r2) throw new Error('R2_ENDPOINT, R2_ACCESS_KEY_ID and R2_SECRET_ACCESS_KEY are required for private custody');
  let present=0,missing=0;
  for(const entry of entries){
    const path=safeDestination(target,entry.path,!check);
    let bytes=existing(path);
    if(bytes===null && !check){
      bytes=objects?existing(safeDestination(objects,entry.key,false)):recovery?existing(safeDestination(recovery,entry.path,false)):await r2(entry);
      if(bytes===null) throw new Error(`private custody object missing: ${entry.key}`);
      verify(entry,bytes);
      const temp=`${path}.private-custody-${process.pid}`;
      try {writeFileSync(temp,bytes,{flag:'wx',mode:0o600}); chmodSync(temp,0o644); linkSync(temp,path); verify(entry,existing(path));}
      finally {try {unlinkSync(temp);} catch(error){if(error.code!=='ENOENT') throw error;}}
    }
    if(bytes===null) missing++; else {verify(entry,bytes); present++;}
  }
  log(`private custody: ${entries.length} manifest entries valid; ${present} files verified; ${missing} in private custody (not hydrated)`);
  return {present,missing};
}

if(process.argv[1] && resolve(process.argv[1])===fileURLToPath(import.meta.url)){
  try {
    const [action,...args]=process.argv.slice(2), options={};
    if(!['check','hydrate'].includes(action)) throw new Error('usage: node bonsai/lib/private-custody.mjs check|hydrate [--target ABS] [--objects ABS|--recovery ABS]');
    for(let i=0;i<args.length;i+=2){if(!['--target','--objects','--recovery'].includes(args[i]) || !args[i+1] || !isAbsolute(args[i+1])) throw new Error('options require absolute paths'); options[args[i].slice(2)]=args[i+1];}
    options.check=action==='check';
    await hydrate(resolve(dirname(fileURLToPath(import.meta.url)),'../..'),options);
  } catch(error){console.error(error.message); process.exitCode=1;}
}
