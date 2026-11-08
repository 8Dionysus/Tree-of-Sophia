#!/usr/bin/env node
// Generated public layers live in R2 (docs/STORAGE.md, bucket tos-public-layers, key sha256/<hex>);
// git keeps only their digest manifest, a layers.json beside its owner: bonsai/<stand>/layers.json for a
// stand's word layer, access/public-corpus/layers.json for the public corpus snapshot (whose `root` names
// the directory of its files). This tool writes and checks a manifest, brings the files to a checkout and
// takes new ones to R2:
//
//   manifest <home>          hash the declared paths and rewrite the manifest's entries (<home> holds layers.json)
//   status <home>            which files are present, missing or different (exit 1 when not all present)
//   fetch <home>             bring missing files: from R2 when R2_* credentials are set, then from the
//                            site's copy (TOS_LAYERS_MIRROR), then from the transfer branch
//   send <home>              put files that no store holds yet on the transfer branch (layers/transfer)
//   upload <home>            put every file R2 lacks into R2 and read it back (R2_* credentials)
//   mirror <home> <dir>      copy the files into <dir>/layers/sha256/<hex>, the site's copy
//   prune <home>...          keep on the transfer branch only what R2 still lacks (R2_* credentials)
//
// Every byte that arrives is checked against its SHA-256 and size before it is written.
import {createHash,createHmac} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {existsSync,mkdirSync,readFileSync,readdirSync,renameSync,statSync,writeFileSync} from 'node:fs';
import {dirname,join,relative,resolve,sep} from 'node:path';
import {fileURLToPath} from 'node:url';

export const LAYERS_SCHEMA='tos.bonsai.layers.v1';
export const TRANSFER_BRANCH='layers/transfer';
export const DEFAULT_MIRROR='https://treeofsophia.com/__research/layers';
const HEX=/^[0-9a-f]{64}$/;

export const sha256=bytes=>createHash('sha256').update(bytes).digest('hex');
export const objectKey=hex=>`sha256/${hex}`;

/** The manifest of a stand's layers, or null where the stand keeps none. */
export function readLayers(home){
  const path=join(home,'layers.json');
  return existsSync(path)?JSON.parse(readFileSync(path,'utf8')):null;
}

/** The directory a manifest's paths are relative to: its own, or the one its `root` names relative to it. */
export const filesRoot=(home,layers)=>resolve(home,layers?.root??'.');

/** Whether a stand-relative path is one of the layer files a manifest declares. */
export const isLayerPath=(layers,path)=>Boolean(layers?.paths?.some(item=>path===item||item.endsWith('/')&&path.startsWith(item)));

/** What is wrong with a manifest's shape; [] when nothing. */
export function layersIssues(layers){
  if(layers?.schema!==LAYERS_SCHEMA)return ['schema'];
  const issues=[];
  if(layers.root!==undefined&&(typeof layers.root!=='string'||!layers.root||layers.root.startsWith('/')))issues.push('root');
  if(!Array.isArray(layers.paths)||!layers.paths.length)issues.push('paths');
  if(!Array.isArray(layers.entries))return [...issues,'entries'];
  const seen=new Set();
  for(const entry of layers.entries){
    if(typeof entry?.path!=='string'||!entry.path||entry.path.startsWith('/')||entry.path.split('/').includes('..'))issues.push(`path ${entry?.path}`);
    else if(!layers.paths.some(path=>entry.path===path||path.endsWith('/')&&entry.path.startsWith(path)))issues.push(`${entry.path}: outside the declared paths`);
    if(!HEX.test(entry?.sha256??''))issues.push(`${entry?.path}: sha256`);
    if(!Number.isInteger(entry?.bytes)||entry.bytes<0)issues.push(`${entry?.path}: bytes`);
    if(seen.has(entry?.path))issues.push(`${entry.path}: twice`);
    seen.add(entry?.path);
  }
  const sorted=[...layers.entries].map(entry=>entry.path).sort();
  if(sorted.some((path,index)=>path!==layers.entries[index].path))issues.push('entries are not in path order');
  return issues;
}

/** The files under the declared paths: a path ending in / is a directory read whole. */
export function declaredFiles(home,paths){
  const files=[];
  const walk=rel=>{
    const full=join(home,rel);
    if(!existsSync(full))throw new Error(`declared layer path is missing: ${rel}`);
    if(statSync(full).isDirectory()){for(const name of readdirSync(full).sort())walk(`${rel.replace(/\/$/,'')}/${name}`);return;}
    files.push(rel);
  };
  for(const path of paths)walk(path);
  return [...new Set(files)].sort();
}

/** A manifest as written: its fields indented, one entry per line, so a regeneration diffs file by file. */
export function formatLayers(layers){
  const {entries,...head}=layers;
  const fields=Object.entries(head).map(([key,value])=>` ${JSON.stringify(key)}: ${JSON.stringify(value)}`);
  const lines=entries.map(entry=>`  ${JSON.stringify(entry)}`);
  return `{\n${[...fields,` "entries": [${lines.length?`\n${lines.join(',\n')}\n `:''}]`].join(',\n')}\n}\n`;
}

/** Rewrite the entries from the files present. */
export function writeManifest(home){
  const layers=readLayers(home);
  if(!layers)throw new Error(`${home} has no layers.json`);
  const root=filesRoot(home,layers);
  const entries=declaredFiles(root,layers.paths).map(path=>{const bytes=readFileSync(join(root,path));return {path,sha256:sha256(bytes),bytes:bytes.length};});
  const next={...layers,entries};
  writeFileSync(join(home,'layers.json'),formatLayers(next));
  return next;
}

const bytesMatch=(bytes,entry)=>bytes&&bytes.length===entry.bytes&&sha256(bytes)===entry.sha256;
function localBytes(home,entry,layers=readLayers(home)){
  const path=join(filesRoot(home,layers),entry.path);
  if(!existsSync(path))return null;
  const bytes=readFileSync(path);
  return bytesMatch(bytes,entry)?bytes:null;
}

/** Whether every file is on disk ('present'), none is ('absent') or some are ('partial'), by existence alone. */
export function layersOnDisk(home,layers=readLayers(home)){
  const root=filesRoot(home,layers),on=layers.entries.filter(entry=>existsSync(join(root,entry.path))).length;
  return on===layers.entries.length?'present':on===0?'absent':'partial';
}

/** Each entry with its state: present, missing or different. */
export function layerStatus(home,layers=readLayers(home)){
  const root=filesRoot(home,layers);
  return layers.entries.map(entry=>{
    const path=join(root,entry.path);
    if(!existsSync(path))return {...entry,state:'missing'};
    return {...entry,state:bytesMatch(readFileSync(path),entry)?'present':'different'};
  });
}

function writeAtomically(path,bytes){
  mkdirSync(dirname(path),{recursive:true});
  const temp=`${path}.part-${process.pid}`;
  writeFileSync(temp,bytes);renameSync(temp,path);
}

// --- R2 through its S3-compatible API, signed with AWS Signature Version 4 -------------------------

const encode=value=>encodeURIComponent(value).replace(/[!'()*]/g,char=>`%${char.charCodeAt(0).toString(16).toUpperCase()}`);
const hmac=(key,value)=>createHmac('sha256',key).update(value).digest();

/** Sign an S3 request: returns the headers to send, Authorization included. */
export function signS3({method,url,headers={},payloadHash,accessKeyId,secretAccessKey,region='auto',date=new Date()}){
  const target=new URL(url);
  const stamp=date.toISOString().replace(/[-:]/g,'').replace(/\.\d{3}/,'');
  const day=stamp.slice(0,8);
  const all={...Object.fromEntries(Object.entries(headers).map(([key,value])=>[key.toLowerCase(),String(value).trim()])),
    host:target.host,'x-amz-content-sha256':payloadHash,'x-amz-date':stamp};
  const names=Object.keys(all).sort();
  const canonicalPath=target.pathname.split('/').map(part=>encode(decodeURIComponent(part))).join('/');
  const query=[...target.searchParams].map(([key,value])=>[encode(key),encode(value)]).sort(([a,x],[b,y])=>a<b?-1:a>b?1:x<y?-1:x>y?1:0).map(([key,value])=>`${key}=${value}`).join('&');
  const canonical=[method,canonicalPath,query,names.map(name=>`${name}:${all[name]}\n`).join(''),names.join(';'),payloadHash].join('\n');
  const scope=`${day}/${region}/s3/aws4_request`;
  const toSign=['AWS4-HMAC-SHA256',stamp,scope,sha256(canonical)].join('\n');
  const key=hmac(hmac(hmac(hmac(`AWS4${secretAccessKey}`,day),region),'s3'),'aws4_request');
  const signature=createHmac('sha256',key).update(toSign).digest('hex');
  return {...all,authorization:`AWS4-HMAC-SHA256 Credential=${accessKeyId}/${scope}, SignedHeaders=${names.join(';')}, Signature=${signature}`};
}

/** R2 from the environment (R2_ENDPOINT, R2_ACCESS_KEY_ID, R2_SECRET_ACCESS_KEY), or null. */
export function r2FromEnv(env=process.env,bucket='tos-public-layers'){
  const {R2_ENDPOINT:endpoint,R2_ACCESS_KEY_ID:accessKeyId,R2_SECRET_ACCESS_KEY:secretAccessKey}=env;
  if(!endpoint||!accessKeyId||!secretAccessKey)return null;
  const base=`${endpoint.replace(/\/+$/,'')}/${bucket}`;
  const request=async(method,hex,body=null)=>{
    const url=`${base}/${objectKey(hex)}`;
    const payloadHash=body?sha256(body):method==='GET'||method==='HEAD'?sha256(Buffer.alloc(0)):'UNSIGNED-PAYLOAD';
    const headers=signS3({method,url,payloadHash,accessKeyId,secretAccessKey,headers:body?{'content-type':'application/octet-stream'}:{}});
    delete headers.host;
    return fetch(url,{method,headers,body});
  };
  return {
    bucket,
    async has(hex){const response=await request('HEAD',hex);if(response.status===404)return false;if(!response.ok)throw new Error(`R2 HEAD ${hex}: ${response.status}`);return true;},
    async get(hex){const response=await request('GET',hex);if(response.status===404)return null;if(!response.ok)throw new Error(`R2 GET ${hex}: ${response.status}`);return Buffer.from(await response.arrayBuffer());},
    async put(hex,bytes){const response=await request('PUT',hex,bytes);if(!response.ok)throw new Error(`R2 PUT ${hex}: ${response.status} ${await response.text()}`);},
  };
}

/** The site's copy of the layers: a plain HTTPS mirror keyed like R2. */
export function mirrorSource(base=process.env.TOS_LAYERS_MIRROR||DEFAULT_MIRROR){
  return {name:'site',async get(hex){
    const response=await fetch(`${base.replace(/\/+$/,'')}/${objectKey(hex)}`).catch(()=>null);
    return response?.ok?Buffer.from(await response.arrayBuffer()):null;
  }};
}

// --- The transfer branch: objects named sha256/<hex>, waiting for R2 -------------------------------

const git=(cwd,args,options={})=>execFileSync('git',args,{cwd,maxBuffer:1<<30,...options});
const TRANSFER_REF='refs/tos-layers/transfer';

/** Fetch the transfer branch into a local ref; false when it does not exist. */
export function fetchTransfer(repo,remote=process.env.TOS_LAYERS_REMOTE||'origin'){
  try{git(repo,['fetch','--quiet','--no-tags','--depth=1',remote,`+refs/heads/${TRANSFER_BRANCH}:${TRANSFER_REF}`],{stdio:['ignore','ignore','pipe']});return true;}
  catch(error){if(/couldn't find remote ref|not found/i.test(String(error.stderr??'')))return false;throw error;}
}
export function transferSource(repo){
  let fetched=null;
  return {name:'transfer',async get(hex){
    fetched??=fetchTransfer(repo);
    if(!fetched)return null;
    try{return git(repo,['cat-file','blob',`${TRANSFER_REF}:${objectKey(hex)}`],{stdio:['ignore','pipe','ignore']});}catch{return null;}
  }};
}
export function transferObjects(repo){
  if(!fetchTransfer(repo))return [];
  const listing=git(repo,['ls-tree','-r','--name-only',TRANSFER_REF],{encoding:'utf8'});
  return listing.split('\n').map(line=>line.match(/^sha256\/([0-9a-f]{64})$/)?.[1]).filter(Boolean);
}

/** Write the transfer branch as one commit holding exactly `blobs` (Map hex → bytes or git blob id) and push it. */
function writeTransfer(repo,objects,{remote=process.env.TOS_LAYERS_REMOTE||'origin',message,expected=null}){
  const lines=[];
  for(const [hex,value] of [...objects].sort(([a],[b])=>a<b?-1:1)){
    const blob=typeof value==='string'?value:git(repo,['hash-object','-w','--stdin'],{input:value,encoding:'utf8'}).trim();
    lines.push(`100644 blob ${blob}\t${hex}`);
  }
  const inner=git(repo,['mktree'],{input:lines.length?`${lines.join('\n')}\n`:'',encoding:'utf8'}).trim();
  const root=git(repo,['mktree'],{input:lines.length?`040000 tree ${inner}\tsha256\n`:'',encoding:'utf8'}).trim();
  const env={...process.env,GIT_AUTHOR_NAME:'Tree of Sophia layers',GIT_AUTHOR_EMAIL:'layers@treeofsophia.invalid',GIT_COMMITTER_NAME:'Tree of Sophia layers',GIT_COMMITTER_EMAIL:'layers@treeofsophia.invalid'};
  const commit=git(repo,['commit-tree',root,'-m',message],{encoding:'utf8',env}).trim();
  const lease=expected?`--force-with-lease=refs/heads/${TRANSFER_BRANCH}:${expected}`:`--force-with-lease=refs/heads/${TRANSFER_BRANCH}:`;
  git(repo,['push','--quiet',lease,remote,`${commit}:refs/heads/${TRANSFER_BRANCH}`],{stdio:['ignore','ignore','inherit']});
  return commit;
}
const transferHead=repo=>{try{return git(repo,['rev-parse',TRANSFER_REF],{encoding:'utf8',stdio:['ignore','pipe','ignore']}).trim();}catch{return null;}};

// --- Commands --------------------------------------------------------------------------------------

/** Bring every missing or different file from the first source that holds its exact bytes. */
export async function fetchLayers(home,{sources,log=()=>{}}){
  const layers=readLayers(home),missing=layerStatus(home,layers).filter(entry=>entry.state!=='present');
  const unresolved=[];
  for(const entry of missing){
    let bytes=null,from=null;
    for(const source of sources){const candidate=await source.get(entry.sha256);if(bytesMatch(candidate,entry)){bytes=candidate;from=source.name;break;}}
    if(!bytes){unresolved.push(entry.path);continue;}
    writeAtomically(join(filesRoot(home,layers),entry.path),bytes);log(`${entry.path} ← ${from}`);
  }
  return {fetched:missing.length-unresolved.length,unresolved};
}

/** Put every file R2 lacks into R2 and read each back. The files must be present. */
export async function uploadLayers(home,{r2,log=()=>{}}){
  const layers=readLayers(home);let uploaded=0;
  for(const entry of layers.entries){
    if(await r2.has(entry.sha256))continue;
    const bytes=localBytes(home,entry,layers);if(!bytes)throw new Error(`${entry.path} is not present with its manifest bytes; fetch it first`);
    await r2.put(entry.sha256,bytes);
    if(!bytesMatch(await r2.get(entry.sha256),entry))throw new Error(`${entry.path}: R2 returned other bytes for ${entry.sha256}`);
    uploaded++;log(`${entry.path} → R2`);
  }
  return {uploaded};
}

/** Copy every file into dir/sha256/<hex>. */
export function mirrorLayers(home,dir){
  const layers=readLayers(home);
  for(const entry of layers.entries){
    const bytes=localBytes(home,entry,layers);if(!bytes)throw new Error(`${entry.path} is not present with its manifest bytes`);
    writeAtomically(join(dir,objectKey(entry.sha256)),bytes);
  }
  return layers.entries.length;
}

/** Put on the transfer branch the files that neither R2 (when reachable), the site nor the branch holds. */
export async function sendLayers(home,{repo,stores,log=()=>{}}){
  const layers=readLayers(home),held=new Set(transferObjects(repo)),head=transferHead(repo);
  const objects=new Map([...held].map(hex=>[hex,git(repo,['rev-parse',`${TRANSFER_REF}:${objectKey(hex)}`],{encoding:'utf8'}).trim()]));
  let added=0;
  for(const entry of layers.entries){
    if(held.has(entry.sha256)||objects.has(entry.sha256))continue;
    let stored=false;for(const store of stores)if(bytesMatch(await store.get(entry.sha256),entry)){stored=true;break;}
    if(stored)continue;
    const bytes=localBytes(home,entry,layers);if(!bytes)throw new Error(`${entry.path} is not present with its manifest bytes`);
    objects.set(entry.sha256,bytes);added++;log(`${entry.path} → ${TRANSFER_BRANCH}`);
  }
  if(added)writeTransfer(repo,objects,{message:`Layers waiting for R2: ${objects.size} objects`,expected:head});
  return {added,waiting:objects.size};
}

/** Keep on the transfer branch only the objects R2 still lacks. */
export async function pruneTransfer(repo,{r2,log=()=>{}}){
  const held=transferObjects(repo),head=transferHead(repo);
  if(!held.length)return {kept:0,removed:0};
  const kept=new Map();
  for(const hex of held)if(!(await r2.has(hex)))kept.set(hex,git(repo,['rev-parse',`${TRANSFER_REF}:${objectKey(hex)}`],{encoding:'utf8'}).trim());
  if(kept.size!==held.length){writeTransfer(repo,kept,{message:`Layers waiting for R2: ${kept.size} objects`,expected:head});log(`${held.length-kept.size} objects now in R2 left ${TRANSFER_BRANCH}`);}
  return {kept:kept.size,removed:held.length-kept.size};
}

async function main([command,...args]){
  const home=args[0]&&resolve(args[0]);
  const repo=home&&git(home,['rev-parse','--show-toplevel'],{encoding:'utf8'}).trim();
  const log=line=>console.log(line);
  const r2=()=>{const client=r2FromEnv();if(!client)throw new Error('R2 credentials are not set (R2_ENDPOINT, R2_ACCESS_KEY_ID, R2_SECRET_ACCESS_KEY)');return client;};
  switch(command){
    case 'manifest':{const layers=writeManifest(home);console.log(`${relative(process.cwd(),join(home,'layers.json'))}: ${layers.entries.length} files`);return 0;}
    case 'status':{
      const issues=layersIssues(readLayers(home));if(issues.length){console.error(issues.join('\n'));return 1;}
      const states=layerStatus(home);const counts={present:0,missing:0,different:0};for(const entry of states)counts[entry.state]++;
      for(const entry of states)if(entry.state!=='present')console.log(`${entry.state}: ${entry.path}`);
      console.log(`${counts.present} present, ${counts.missing} missing, ${counts.different} different`);
      return counts.present===states.length?0:1;
    }
    case 'fetch':{
      const client=r2FromEnv();
      const sources=[...(client?[{name:'R2',get:hex=>client.get(hex)}]:[]),mirrorSource(),transferSource(repo)];
      const {fetched,unresolved}=await fetchLayers(home,{sources,log});
      console.log(`${fetched} files fetched`);
      if(unresolved.length){console.error(`not found in any store:\n${unresolved.join('\n')}`);return 1;}
      return 0;
    }
    case 'send':{
      const client=r2FromEnv();
      const stores=[...(client?[{name:'R2',get:hex=>client.get(hex)}]:[]),mirrorSource()];
      const {added,waiting}=await sendLayers(home,{repo,stores,log});
      console.log(`${added} files sent; ${waiting} objects wait on ${TRANSFER_BRANCH}`);return 0;
    }
    case 'upload':{const {uploaded}=await uploadLayers(home,{r2:r2(),log});console.log(`${uploaded} files uploaded to R2`);return 0;}
    case 'mirror':{if(!args[1])throw new Error('mirror needs a target directory');const count=mirrorLayers(home,resolve(args[1]));console.log(`${count} files copied`);return 0;}
    case 'prune':{const {kept,removed}=await pruneTransfer(repo,{r2:r2(),log});console.log(`${removed} removed, ${kept} still wait on ${TRANSFER_BRANCH}`);return 0;}
    default:
      console.error('usage: layers.mjs manifest|status|fetch|send|upload|prune <bonsai home> | mirror <bonsai home> <dir>');
      return 2;
  }
}

if(process.argv[1]&&resolve(process.argv[1])===fileURLToPath(import.meta.url))
  main(process.argv.slice(2)).then(code=>process.exit(code),error=>{console.error(error.message);process.exit(1);});
