#!/usr/bin/env node
// Run the readers of the Population line. The operator's tool: a reader never starts it.
//
//   node bonsai/review/population/line/drive.mjs [--count N] [--parallel P] [--packets p-0001,p-0002]
//
// For each packet that is not accepted: two fresh `codex exec` sessions in the packet's folder, reader 1 and
// reader 2, at the same time. Each writes its verdicts to a file of its own outside the packet
// (<logs>/<packet>/verdicts/reader-N.jsonl), so neither reader's file stands where the other works; each session
// is told only its own path. The gate (kit.mjs gate --reader N --file F) decides whether a reader is done; while it
// refuses a reader's verdicts, that reader's session is resumed with the gate's own lines, twice at most. Once both
// readers pass, both files are laid in the packet, and where the whole gate opens, kit.mjs accept writes the agreed
// verdicts as decisions. A usage or rate limit stops the whole line; a fault of the line (exit 3) stops it too.
//
// Environment: CODEX (binary, default ~/.local/bin/codex), TOS_POPULATION_MODEL (default gpt-6.1-terra),
// TOS_POPULATION_EFFORT (default high), TOS_POPULATION_OUT (logs; default ~/.local/share/claude-codex/out/population).

import {spawn,spawnSync} from 'node:child_process';
import {copyFileSync,createWriteStream,existsSync,mkdirSync,readFileSync,readdirSync} from 'node:fs';
import {homedir} from 'node:os';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';

const LINE=dirname(fileURLToPath(import.meta.url));
const PACKETS=join(LINE,'packets');
const KIT=join(LINE,'kit.mjs');
const CODEX=process.env.CODEX??join(homedir(),'.local','bin','codex');
const MODEL=process.env.TOS_POPULATION_MODEL??'gpt-6.1-terra';
const EFFORT=process.env.TOS_POPULATION_EFFORT??'high';
const OUT=process.env.TOS_POPULATION_OUT??join(homedir(),'.local','share','claude-codex','out','population');
const TURN=90*60*1000;
const RESUMES=2;
let stopped=null;

const kit=(...args)=>{const run=spawnSync(process.execPath,[KIT,...args],{encoding:'utf8'});return {code:run.status,text:`${run.stdout}${run.stderr}`.trim()};};
// Where a reader's verdicts wait until both readers pass their gates: outside the packet, under the logs.
const verdicts=(packet,reader)=>join(OUT,packet,'verdicts',`reader-${reader}.jsonl`);
const ownGate=(packet,reader)=>kit('gate',packet,'--reader',String(reader),'--file',verdicts(packet,reader));

function prompt(reader,file){
  return `You are reader ${reader} of the Population line of the Tree of Sophia. Read ../../AGENTS.md and ../../READER.md first (packet.json names the questions' type, and READER.md has its section), then the questions: node ../../kit.mjs show ${'$'}(basename "$PWD"). `+
    `Answer every question in ${file}, one JSON line per question, as READER.md says; that file is yours alone, and the other reader's verdicts are nowhere you are asked to look. `+
    `When you are done, run: node ../../kit.mjs gate ${'$'}(basename "$PWD") --reader ${reader} --file ${file} — and fix your file until it names no problem. Stop then.`;
}

function codex(packet,reader,text,thread=null){
  return new Promise(resolve=>{
    const home=join(OUT,packet);mkdirSync(home,{recursive:true});
    const n=readdirSync(home).filter(name=>name.endsWith('.jsonl')).length+1;
    const log=join(home,`${String(n).padStart(2,'0')}-reader-${reader}.jsonl`);
    const args=['exec',...(thread?['resume',thread]:[]),'--json','-m',MODEL,'-c',`model_reasoning_effort=${EFFORT}`,'--dangerously-bypass-approvals-and-sandbox','--skip-git-repo-check','-o',log.replace(/\.jsonl$/,'.md'),text];
    const child=spawn(CODEX,args,{cwd:join(PACKETS,packet),stdio:['ignore','pipe','pipe']});
    const file=createWriteStream(log);child.stdout.pipe(file);child.stderr.pipe(file);
    const timer=setTimeout(()=>child.kill('SIGTERM'),TURN);
    child.on('close',()=>{
      clearTimeout(timer);
      let found=thread,limit=false;
      for(const line of readFileSync(log,'utf8').split('\n')){
        let event;try{event=JSON.parse(line);}catch{continue;}
        if(event.type==='thread.started')found=event.thread_id;
        const message=['error','turn.failed'].includes(event.type)?JSON.stringify(event.error??event.message??''):'';
        if(/usage limit|rate limit/i.test(message))limit=true;
      }
      resolve({thread:found,limit});
    });
  });
}

async function reader(packet,number){
  const file=verdicts(packet,number);mkdirSync(dirname(file),{recursive:true});
  let session=await codex(packet,number,prompt(number,file));
  for(let turn=0;turn<RESUMES&&!session.limit;turn++){
    const gate=ownGate(packet,number);
    if(gate.code!==1)break;
    const mine=gate.text.split('\n').filter(line=>line.startsWith(`reader-${number}`));
    session=await codex(packet,number,`The gate refused these verdicts of yours:\n${mine.slice(0,60).join('\n')}\nFix ${file} and run the gate again with --reader ${number} --file ${file}.`,session.thread);
  }
  if(session.limit)stopped='the usage limit';
}

async function runPacket(packet){
  if(stopped||existsSync(join(PACKETS,packet,'accepted.json')))return;
  // Verdicts a reader left in the packet before the files were kept apart wait outside it like the others.
  for(const number of [1,2]){const inside=join(PACKETS,packet,`reader-${number}.jsonl`),file=verdicts(packet,number);if(existsSync(inside)&&!existsSync(file)){mkdirSync(dirname(file),{recursive:true});copyFileSync(inside,file);}}
  await Promise.all([1,2].filter(number=>!existsSync(verdicts(packet,number))||ownGate(packet,number).code===1).map(number=>reader(packet,number)));
  if(stopped)return;
  const own=[1,2].map(number=>ownGate(packet,number));
  const fault=own.find(gate=>gate.code===3);
  if(fault){stopped=`a fault of the line in ${packet}: ${fault.text}`;return;}
  if(own.some(gate=>gate.code!==0)){console.log(`${packet}: a reader's gate is shut\n${own.map(gate=>gate.text).join('\n').split('\n').slice(0,6).join('\n')}`);return;}
  // Both readers have passed: their verdicts are laid in the packet together, and only then compared.
  for(const number of [1,2])copyFileSync(verdicts(packet,number),join(PACKETS,packet,`reader-${number}.jsonl`));
  const gate=kit('gate',packet);
  if(gate.code===3){stopped=`a fault of the line in ${packet}: ${gate.text}`;return;}
  if(gate.code!==0){console.log(`${packet}: gate shut\n${gate.text.split('\n').slice(0,6).join('\n')}`);return;}
  const accepted=kit('accept',packet,'--by',`drive:${MODEL}:${EFFORT}`);
  console.log(accepted.text);
}

async function main(args){
  const option=name=>{const at=args.indexOf(name);return at>=0?args[at+1]:null;};
  const parallel=Math.max(1,Math.min(16,Number(option('--parallel')??4)));
  const chosen=option('--packets')?.split(',')??readdirSync(PACKETS).filter(name=>/^p-\d{4}$/.test(name)).sort()
    .filter(packet=>!existsSync(join(PACKETS,packet,'accepted.json'))&&!existsSync(join(PACKETS,packet,'retired.json')));
  const queue=chosen.slice(0,Number(option('--count')??chosen.length));
  const workers=Array.from({length:parallel},async()=>{while(queue.length&&!stopped)await runPacket(queue.shift());});
  await Promise.all(workers);
  if(stopped){console.log(`line stopped: ${stopped}`);process.exit(3);}
  console.log(kit('status').text);
}

if(process.argv[1]===fileURLToPath(import.meta.url))await main(process.argv.slice(2));
