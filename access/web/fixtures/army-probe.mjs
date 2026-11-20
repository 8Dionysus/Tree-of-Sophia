// Development fixture: fills a separate act journal in this browser with the army simulator's stream and reports
// how long writing, reloading and folding take. It uses its own database name and never the reader's journal.
import initRules,* as rules from '../../deploy/cloudflare-worker/generated/tos_web_rules.js';
import {installActJournalRules,createActJournal} from '../src/act-journal/journal.mjs';
import {openActStore} from '../src/act-journal/store.mjs';
import {simulateArmy} from './army-simulator.mjs';

await initRules(new URL('../../deploy/cloudflare-worker/generated/tos_web_rules_bg.wasm',import.meta.url));
installActJournalRules(rules);

const round=value=>Math.round(value*10)/10;
const deleteDatabase=name=>new Promise(resolve=>{const request=indexedDB.deleteDatabase(name);request.onsuccess=request.onerror=request.onblocked=()=>resolve();});

/** Run one size. Returns timings in milliseconds. */
export async function probeArmy({agents=50,acts=10000,chunk=1000,name=`tos-act-journal-probe-${agents}-${acts}`}={}){
  await deleteDatabase(name);
  const store=await openActStore({name});
  const journal=createActJournal({store,name,channel:null});await journal.ready;
  let written=0;const started=performance.now();
  for(let part=0;written<acts;part++){
    const size=Math.min(chunk,acts-written);
    const {acts:drafts}=simulateArmy({agents,acts:size,seed:500+part,start:Date.UTC(2026,10,20,9)+written*60});
    for(const draft of drafts){draft.id=`p${part}-${draft.id}`;draft.parents=draft.parents.map(id=>`p${part}-${id}`);draft.targets=draft.targets.map(t=>t.startsWith('act:')?`act:p${part}-${t.slice(4)}`:t);}
    await journal.appendMany(drafts);written+=size;
  }
  const writeMs=performance.now()-started;
  journal.dispose();
  const reloadStarted=performance.now();
  const again=createActJournal({store:await openActStore({name}),name,channel:null});const status=await again.ready;
  const reloadMs=performance.now()-reloadStarted;
  const timed=async request=>{const at=performance.now();await again.query(request);return round(performance.now()-at);};
  const now=Date.UTC(2026,10,20,9)+acts*60;
  const result={agents,acts,store:store.kind,write_ms:Math.round(writeMs),write_per_act_us:round(writeMs*1000/acts),reload_ms:Math.round(reloadMs),
    reload_per_act_us:round(reloadMs*1000/acts),damaged:status.damaged,loaded:status.loaded,
    review_ms:await timed({view:'review',limit:50}),board_ms:await timed({view:'board',limit:100}),presence_ms:await timed({view:'presence',now_ms:now,window_ms:600000}),
    recent_ms:await (async()=>{const at=performance.now();await again.recent({limit:100});return round(performance.now()-at);})(),
    storage:await navigator.storage?.estimate?.().then(value=>({usage_mb:round(value.usage/1e6)})).catch(()=>null)};
  again.dispose();await deleteDatabase(name);
  return result;
}

const output=document.querySelector('#army-output');
document.querySelector('#army-run')?.addEventListener('click',async()=>{
  const agents=Number(document.querySelector('#army-agents').value),acts=Number(document.querySelector('#army-acts').value);
  output.textContent='…';output.textContent=JSON.stringify(await probeArmy({agents,acts}),null,2);
});
globalThis.probeArmy=probeArmy;
