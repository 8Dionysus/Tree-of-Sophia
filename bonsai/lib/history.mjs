#!/usr/bin/env node
import {existsSync,mkdirSync,readFileSync,writeFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {join,resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {canonicalJSON} from './history-format.mjs';

export const CORE_SNAPSHOT_SCHEMA='tos_bonsai_research_snapshot_v1';
export const CURRENT_REVISION_SCHEMA='tos_bonsai_research_revision_v1';
export const CORE_SNAPSHOT_PROFILE='zarathustra';

const CORE_FILES=Object.freeze([
  {key:'manifest',file:'core.json',path:'bonsai/zarathustra/core/core.json'},
  {key:'concepts',file:'concepts.json',path:'bonsai/zarathustra/core/concepts.json'},
  {key:'figures',file:'figures.json',path:'bonsai/zarathustra/core/figures.json'},
  {key:'links',file:'links.json',path:'bonsai/zarathustra/core/links.json'},
  {key:'chains',file:'chains.json',path:'bonsai/zarathustra/core/chains.json'},
  {key:'proposed',file:'proposed.json',path:'bonsai/zarathustra/core/proposed.json'},
  {key:'voices',file:'voices.json',path:'bonsai/zarathustra/core/voices.json'},
  {key:'placeRelations',file:'place-relations.json',path:'bonsai/zarathustra/core/place-relations.json'},
  {key:'semanticUses',file:'semantic-uses.json',path:'bonsai/zarathustra/core/semantic-uses.json'},
]);

function sameBytes(path,expected) {
  return existsSync(path) && readFileSync(path).equals(expected);
}

function preservedSnapshotBytes(path,existingBytes,packet,coreCanonical,metadataCanonical) {
  let existing;
  try {
    existing=JSON.parse(existingBytes.toString('utf8'));
  } catch (error) {
    throw new Error(`Existing historical snapshot is not valid JSON at ${path}: ${error.message}`);
  }
  const fields=['schema','profile','source_commit','core','metadata','revision'];
  if (!existing || typeof existing !== 'object' || Array.isArray(existing)
    || Object.keys(existing).length !== fields.length || fields.some(key => !Object.hasOwn(existing,key)))
    throw new Error(`Existing historical snapshot has an invalid envelope at ${path}`);
  if (existing.schema !== CORE_SNAPSHOT_SCHEMA || existing.profile !== CORE_SNAPSHOT_PROFILE
    || existing.revision !== packet.revision)
    throw new Error(`Existing historical snapshot identity differs at ${path}`);
  if (typeof existing.source_commit !== 'string' || !existing.source_commit.trim()
    || existing.source_commit !== existing.source_commit.trim())
    throw new Error(`Existing historical snapshot source_commit is invalid at ${path}`);
  if (canonicalJSON(existing.core) !== coreCanonical)
    throw new Error(`Existing historical snapshot core differs at ${path}`);
  if (canonicalJSON(existing.metadata) !== metadataCanonical)
    throw new Error(`Existing historical snapshot metadata differs at ${path}`);

  const firstPacket={...packet,source_commit:existing.source_commit};
  const firstBytes=Buffer.from(JSON.stringify(firstPacket),'utf8');
  if (!existingBytes.equals(firstBytes))
    throw new Error(`Existing historical snapshot bytes differ at ${path}`);
  return {sourceCommit:existing.source_commit,bytes:existingBytes};
}

function writeOrReuseRevisionFile(path,bytes,packet,coreCanonical,metadataCanonical) {
  try {
    writeFileSync(path,bytes,{flag:'wx'});
    return {sourceCommit:packet.source_commit,bytes};
  } catch (error) {
    if (error?.code !== 'EEXIST') throw error;
    return preservedSnapshotBytes(path,readFileSync(path),packet,coreCanonical,metadataCanonical);
  }
}

/** Capture the current Zarathustra core as a content-addressed historical packet. */
export function captureCoreSnapshot(root,{sourceCommit}={}) {
  if (typeof root !== 'string' || !root.trim()) throw new TypeError('A bonsai root path is required.');
  if (typeof sourceCommit !== 'string' || !sourceCommit.trim() || sourceCommit !== sourceCommit.trim())
    throw new TypeError('A trimmed source commit is required.');

  const standRoot=resolve(root),coreDir=join(standRoot,'core');
  if (!existsSync(coreDir)) throw new Error(`The core directory does not exist: ${coreDir}`);

  const core={},missingMembers=[];
  for (const item of CORE_FILES) {
    const path=join(coreDir,item.file);
    let source;
    try {
      source=readFileSync(path,'utf8');
    } catch (error) {
      if (error?.code !== 'ENOENT') throw error;
      core[item.key]=null;
      missingMembers.push(item.path);
      continue;
    }
    try {
      core[item.key]=JSON.parse(source);
    } catch (error) {
      throw new SyntaxError(`Invalid JSON in ${path}: ${error.message}`);
    }
  }

  const coreCanonical=canonicalJSON(core);
  const revision=createHash('sha256').update(coreCanonical,'utf8').digest('hex');
  const metadata={missing_members:missingMembers};
  const metadataCanonical=canonicalJSON(metadata);
  const packet={
    schema:CORE_SNAPSHOT_SCHEMA,
    profile:CORE_SNAPSHOT_PROFILE,
    source_commit:sourceCommit,
    core,
    metadata,
    revision,
  };
  const candidateBytes=Buffer.from(JSON.stringify(packet),'utf8');
  const historyDir=join(standRoot,'history');
  mkdirSync(historyDir,{recursive:true});
  const snapshotPath=join(historyDir,`${revision}.json`);
  const stored=writeOrReuseRevisionFile(snapshotPath,candidateBytes,packet,coreCanonical,metadataCanonical);

  const currentPath=join(coreDir,'history-current.json');
  const currentBytes=Buffer.from(JSON.stringify({
    schema:CURRENT_REVISION_SCHEMA,
    revision,
    source_commit:stored.sourceCommit,
  }),'utf8');
  if (!sameBytes(currentPath,currentBytes)) writeFileSync(currentPath,currentBytes);

  return {revision,path:snapshotPath,bytes:stored.bytes.byteLength};
}

function runCli(args) {
  const [command,checkoutRootArg,...extra]=args;
  if (command !== 'capture' || extra.length) {
    console.error('Usage: node bonsai/lib/history.mjs capture [checkoutroot]');
    process.exitCode=2;
    return;
  }
  const moduleRoot=resolve(dirname(fileURLToPath(import.meta.url)),'../..');
  const checkoutRoot=resolve(checkoutRootArg??moduleRoot);
  const sourceCommit=execFileSync('git',['rev-parse','HEAD'],{cwd:checkoutRoot,encoding:'utf8'}).trim();
  const result=captureCoreSnapshot(join(checkoutRoot,'bonsai','zarathustra'),{sourceCommit});
  process.stdout.write(`${JSON.stringify(result)}\n`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    runCli(process.argv.slice(2));
  } catch (error) {
    console.error(error?.stack ?? String(error));
    process.exitCode=1;
  }
}
