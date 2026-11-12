#!/usr/bin/env node
// Check a bonsai against the Tree, or write its export packet.
//
//   node bonsai/lib/cli.mjs check [<bonsai>...]
//   node bonsai/lib/cli.mjs export <bonsai> [--out <file>]
//   node bonsai/lib/cli.mjs align <bonsai> [--write]
//   node bonsai/lib/cli.mjs align-corpus <bonsai> [--write] [--corpus <dir>]
//   node bonsai/lib/cli.mjs media add <bonsai> <observation.json> --subject <id> --role <role> --artifact <tradition/site/identity> [--view <view>]
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
// `media add` attaches a medium to a card: it reads a provider observation made by
// the Tree's media bridge (`tos-native-owner-command acquisition`, `media.observe`),
// stores its work and image in the bonsai's media module with a use for the
// subject in the role, names their Tree Artifact and representation in the graft
// map, and checks the bonsai. A file image's file and smaller copy are fetched
// from the provider, checked against the observation's fingerprints, kept under
// the stand's layer store (`media/`) and entered in its layers.json; `layers.mjs
// send` or the Publish Bonsai workflow takes them on to R2.

import {readFileSync,writeFileSync,readdirSync,existsSync,mkdirSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {loadCore} from './core.mjs';
import {exportPhilosophy} from './export.mjs';
import {dirname,join} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {alignCorpus,citedUnits,corpusReader,formatAlignment} from './corpus-align.mjs';
import {translationScopesIssues} from './fragments.mjs';
import {bonsaiIds,checkBonsai,loadBonsai,loadWitnessChapters} from './graft.mjs';
import {realignWitnesses} from './realign.mjs';
import {addMedia,MEDIA_LAYER_PATH,mediaFilePath} from './media.mjs';
import {formatLayers,readLayers,sha256} from './layers.mjs';

const root=join(dirname(fileURLToPath(import.meta.url)),'..','..');
const [command,...rest]=process.argv.slice(2);
const option=name=>{const index=rest.indexOf(name);return index>=0?rest[index+1]:null;};
const names=rest.filter((value,index)=>!value.startsWith('--')&&!(index>0&&rest[index-1].startsWith('--')));

if(command==='check'){
  let failed=false;
  for(const id of names.length?names:bonsaiIds(root)){
    const result=checkBonsai(root,id);
    const counts=result.packet?`${result.packet.claims.length} claims, ${result.packet.records.length} records, ${result.packet.identities.length} identities`:'no export';
    console.log(`${id}: ${result.problems.length?'FAILED':'ok'} · ${counts}${result.checkedIdentities?'':' · Tree identities not checked out'}${result.layers==='absent'?' · layers not fetched, their manifest checked':''}${result.corpus==='absent'?' · corpus not fetched, its places unchecked':''}`);
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
}else if(command==='export-philosophy'&&names.length===1){
  const {home,manifest:initialManifest}=loadBonsai(root,names[0]);
  const entry=(initialManifest.modules??[]).find(item=>item.id==='core');
  if(!entry)throw new Error(`${names[0]} has no philosophical core`);
  const dir=join(home,dirname(entry.path));
  const relative=path=>path.slice(root.length+1).replaceAll('\\','/');
  const sources={};
  const add=path=>{sources[relative(path)]=`sha256:${createHash('sha256').update(readFileSync(path)).digest('hex')}`;};
  const walk=path=>{for(const entry of readdirSync(path,{withFileTypes:true}).sort((a,b)=>a.name.localeCompare(b.name))){const child=join(path,entry.name);if(entry.isDirectory())walk(child);else if(entry.isFile()&&entry.name.endsWith('.json'))add(child);}};
  add(join(home,'bonsai.json'));for(const id of ['identities','objects','witnesses','fragments','alignment']){const item=(initialManifest.modules??[]).find(item=>item.id===id);if(item?.path)add(join(home,item.path));}walk(dir);
  const chapterDir=join(home,'witnesses');if(existsSync(chapterDir))walk(chapterDir);
  const history={},historyDir=join(home,'history');
  if(existsSync(historyDir))for(const name of readdirSync(historyDir).filter(name=>name.endsWith('.json')).sort()){const path=join(historyDir,name);add(path);history[name]=JSON.parse(readFileSync(path,'utf8'));}
  const operationsPath=join(home,'research-operations.json');
  const hasOperations=existsSync(operationsPath);if(hasOperations)add(operationsPath);
  const researchOperations=hasOperations?JSON.parse(readFileSync(operationsPath,'utf8')):null;
  const {manifest,modules}=loadBonsai(root,names[0]),core=loadCore(dir);
  for(const [path,digest] of Object.entries(sources)){const actual=`sha256:${createHash('sha256').update(readFileSync(join(root,path))).digest('hex')}`;if(actual!==digest)throw new Error(`Source changed during export: ${path}`);}
  const revision={commit:execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim(),
    note:'The digests bind exact working-tree input bytes; the commit alone does not include uncommitted changes.'};
  const packet=exportPhilosophy({manifest,core,identities:modules.identities??{},sources,revision,history,researchOperations,objects:modules.objects??null,witnesses:modules.witnesses??null,fragments:modules.fragments??null,alignment:modules.alignment??null});
  const text=`${JSON.stringify(packet,null,2)}\n`,out=option('--out');
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
  // Held material is preserved for later growth, not cited by the stand.
  const content=[];
  for(const item of manifest.modules??[]){
    if(item.borrowed){for(const path of [].concat(item.borrowed))if(path.endsWith('.mjs'))content.push(Object.values(await import(pathToFileURL(join(root,path)).href)));}
    else if(!['alignment','held'].includes(item.id)&&modules[item.id])content.push(modules[item.id]);
  }
  const works=Object.fromEntries(Object.entries(modules.identities?.works??{}).map(([key,value])=>[key,value.tree]));
  if(modules['translation-scopes']){const issues=translationScopesIssues(modules['translation-scopes']);if(issues.length)throw new Error(`translation scopes: ${issues.join(', ')}`);}
  const read=corpusReader(join(root,option('--corpus')??'access/web/public/public-corpus'));
  const text=formatAlignment(alignCorpus(citedUnits(content,works,read),works,read,modules['translation-scopes']?.scopes??[])),path=join(home,module.path);
  const same=readFileSync(path,'utf8')===text;
  console.log(`${names[0]}: the stored corpus alignment ${same?'matches':'differs from'} the one the mechanism gives`);
  if(rest.includes('--write')){writeFileSync(path,text);console.log('stored');}
  else if(!same)process.exit(1);
}else if(command==='media'&&names[0]==='add'&&names.length===3){
  // The manifest is read alone: the media module it names may not exist before the first medium.
  const [,stand,file]=names,home=join(root,'bonsai',stand),manifest=JSON.parse(readFileSync(join(home,'bonsai.json'),'utf8'));
  const entry=(manifest.modules??[]).find(item=>item.id==='media'),graftEntry=(manifest.modules??[]).find(item=>item.id==='identities');
  if(!entry||!graftEntry){console.error(`${stand}: its manifest names no ${entry?'identities':'media'} module; add it to bonsai/${stand}/bonsai.json first`);process.exit(1);}
  const read=path=>existsSync(path)?JSON.parse(readFileSync(path,'utf8')):null,write=(path,value)=>writeFileSync(path,`${JSON.stringify(value,null,2)}\n`);
  const mediaPath=join(home,entry.path),graftPath=join(home,graftEntry.path);
  const result=addMedia({
    module:read(mediaPath),identities:read(graftPath),observation:JSON.parse(readFileSync(file,'utf8')),bonsai:stand,
    subject:option('--subject'),role:option('--role'),artifact:option('--artifact'),view:option('--view')??'whole',
    digest:text=>createHash('sha256').update(text,'utf8').digest('hex'),
  });
  let layers=null;
  const copies=new Map();
  if(result.image.api==='file'){
    layers=readLayers(home);
    if(!layers||layers.root)throw new Error(`${stand}: file media needs a layer store beside the manifest`);
    if(!layers.paths.includes(MEDIA_LAYER_PATH))layers.paths.push(MEDIA_LAYER_PATH);
    const profile=JSON.parse(readFileSync(join(root,'access/profiles/deployment-profile.default.v1.json'),'utf8'));
    const limit=profile.admission.acquisition.max_record_bytes;
    for(const file of [result.image.preview,result.image.file]){
      const url=new URL(file.source);
      if(url.origin!=='https://images.metmuseum.org'||url.username||url.password
        ||!Number.isSafeInteger(file.bytes)||file.bytes<=0||file.bytes>limit||!/^[a-f0-9]{64}$/.test(file.sha256))
        throw new Error('invalid or out-of-bounds media file descriptor');
      const path=mediaFilePath(file),full=join(home,path);
      copies.set(full,file);
      layers.entries=[...layers.entries.filter(entry=>entry.path!==path),{path,sha256:file.sha256,bytes:file.bytes}]
        .sort((a,b)=>a.path<b.path?-1:a.path>b.path?1:0);
    }
  }
  const check=checkBonsai(root,stand,{overrides:{media:result.module,identities:result.identities},mediaLayers:layers});
  for(const problem of check.problems)console.error(`  problem: ${problem}`);
  if(check.problems.length)process.exit(1);
  const pending=[];
  for(const [full,file] of copies){
    if(existsSync(full)){
      const bytes=readFileSync(full);
      if(bytes.length===file.bytes&&sha256(bytes)===file.sha256)continue;
      throw new Error(`${full}: existing media does not match its fingerprint`);
    }
    const response=await fetch(file.source,{redirect:'error',signal:AbortSignal.timeout(45000),
      headers:{'User-Agent':'Tree-of-Sophia-bonsai-media/1',Accept:'image/jpeg, image/png;q=0.9, image/*;q=0.8'}});
    if(!response.ok||!response.body)throw new Error(`${file.source}: HTTP ${response.status}`);
    const chunks=[];let size=0;
    for await(const chunk of response.body){
      size+=chunk.length;
      if(size>file.bytes)throw new Error(`${file.source}: response exceeds its fingerprinted size`);
      chunks.push(chunk);
    }
    const bytes=Buffer.concat(chunks,size);
    if(size!==file.bytes||sha256(bytes)!==file.sha256)throw new Error(`${file.source}: response differs from the observation`);
    pending.push([full,bytes]);
  }
  for(const [full,bytes] of pending){
    mkdirSync(dirname(full),{recursive:true});writeFileSync(full,bytes,{flag:'wx'});
  }
  if(layers)writeFileSync(join(home,'layers.json'),formatLayers(layers));
  write(mediaPath,result.module);write(graftPath,result.identities);
  console.log(`${stand}: ${result.imageId} shows ${option('--subject')} as ${option('--role')}; work ${result.workId}`);
}else{
  console.error('usage: node bonsai/lib/cli.mjs check [<bonsai>...] | export <bonsai> [--out <file>] | export-philosophy <bonsai> [--out <file>] | align <bonsai> [--write] | align-corpus <bonsai> [--write] [--corpus <dir>] | media add <bonsai> <observation.json> --subject <id> --role <role> --artifact <tradition/site/identity> [--view <view>]');
  process.exit(2);
}
