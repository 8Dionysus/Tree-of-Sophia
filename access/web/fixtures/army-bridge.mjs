#!/usr/bin/env node
// Development fixture: drives a running bridge (`tos bridge`) with the army simulator's stream and reports what it
// measured. The person's acts go through the page's interface (with the page's cookie), the agents' acts through
// MCP from many clients at once, as an army would send them. The records are synthetic: use a store of their own.
//
//   node access/web/fixtures/army-bridge.mjs http://127.0.0.1:8742 [--agents 500] [--acts 20000] [--clients 100]
import {simulateArmy} from './army-simulator.mjs';

const args=process.argv.slice(2);
const base=(args.find(arg=>/^https?:\/\//.test(arg))??'http://127.0.0.1:8742').replace(/\/$/,'');
const option=(name,fallback)=>{const at=args.indexOf(`--${name}`);return at>=0?Number(args[at+1]):fallback;};
const agents=option('agents',500),total=option('acts',20000),clients=option('clients',100),seed=option('seed',11);

const page=await fetch(`${base}/zarathustra/`);
const cookie=(page.headers.get('set-cookie')??'').split(';')[0];
if(!cookie.startsWith('tos_bridge='))throw new Error('the bridge set no cookie: is it serving the built site?');
const person=(path,body)=>fetch(`${base}/__bridge/v1/${path}`,{method:body===undefined?'GET':'POST',headers:{cookie,'content-type':'application/json'},body:body===undefined?undefined:JSON.stringify(body)}).then(response=>response.json());

let rpc=0;
async function mcp(name,input){
  const response=await fetch(`${base}/mcp`,{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({jsonrpc:'2.0',id:++rpc,method:'tools/call',params:{name,arguments:input}})});
  const answer=await response.json();
  return {error:answer.result?.isError===true,value:JSON.parse(answer.result?.content?.[0]?.text??'null')};
}

// The page follows the journal through one event stream; count what it hears while the army works.
let heard=0;const stream=new AbortController();
void fetch(`${base}/__bridge/v1/events`,{headers:{cookie},signal:stream.signal}).then(async response=>{
  const reader=response.body.getReader();for(;;){const {done,value}=await reader.read();if(done)break;heard+=(new TextDecoder().decode(value).match(/event: acts/g)??[]).length;}
}).catch(()=>{});

const {acts}=simulateArmy({agents,acts:total,seed,start:Date.now(),stepMs:5});
const latencies=[],refusals={};let appended=0,retries=0;
const started=performance.now();
for(let at=0;at<acts.length;){
  if(acts[at].author.type==='human'){
    // The person's acts keep their order: a review names the proposals before it.
    let end=at;while(end<acts.length&&acts[end].author.type==='human')end++;
    const out=await person('acts',acts.slice(at,end));
    appended+=out.appended.length;if(out.failure)refusals[out.failure.error]=(refusals[out.failure.error]??0)+1;
    at=end;continue;
  }
  let end=at;while(end<acts.length&&acts[end].author.type!=='human')end++;
  const run=acts.slice(at,end).map(({id,kind,author,targets,grounds,intent,body,parents})=>({id,kind,author,targets,grounds,intent,body,parents}));
  let next=0;
  await Promise.all(Array.from({length:Math.min(clients,run.length)},async()=>{
    while(next<run.length){
      const item=run[next++];
      for(;;){
        const t0=performance.now();const out=await mcp('tos.act.append',item);latencies.push(performance.now()-t0);
        if(!out.error){appended++;break;}
        refusals[out.value.error]=(refusals[out.value.error]??0)+1;
        if(out.value.error==='bridge_pace'||out.value.error==='bridge_queue_full'){retries++;await new Promise(resolve=>setTimeout(resolve,Math.min(out.value.retry_after_ms??100,2000)));if(retries<total)continue;}
        break;
      }
    }
  }));
  at=end;
}
const seconds=(performance.now()-started)/1000;
latencies.sort((a,b)=>a-b);
const pick=q=>Math.round(latencies[Math.min(latencies.length-1,Math.floor(q*latencies.length))]*10)/10;
const timed=async request=>{const t0=performance.now();await person('query',request);return Math.round((performance.now()-t0)*10)/10;};
const now=Date.now();
const folds={summary:await timed({view:'summary'}),board:await timed({view:'board',limit:100}),review:await timed({view:'review',limit:100}),
  squads:await timed({view:'squads',now_ms:now,window_ms:900000,limit:100}),presence:await timed({view:'presence',now_ms:now,window_ms:900000,limit:100})};
const t0=performance.now();const window=await person(`acts?before=${Number.MAX_SAFE_INTEGER}&limit=500`);const windowMs=Math.round((performance.now()-t0)*10)/10;
await new Promise(resolve=>setTimeout(resolve,300));stream.abort();
console.log(JSON.stringify({agents,acts:total,clients,appended,seconds:Math.round(seconds*10)/10,acts_per_second:Math.round(appended/seconds),
  mcp_ms:{p50:pick(.5),p95:pick(.95),max:pick(1)},refusals,retries,events_heard:heard,folds_ms:folds,window_500_ms:windowMs,last:window.last},null,1));
