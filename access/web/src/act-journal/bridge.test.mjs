import '../observatory/human-forms-wasm-test-runtime.mjs';
import {test,expect} from 'vitest';
import * as runtime from '../../../deploy/cloudflare-worker/generated/tos_web_rules.js';
import registry from '../../../contracts/act-kinds.v1.json' with {type:'json'};
import {createBridgeJournal,createBridgeStore,detectBridge,openActJournal} from './bridge.mjs';
import {draftAct} from './drafts.mjs';
import {agentDraft} from './agent-tools.mjs';

const encode=value=>new TextEncoder().encode(JSON.stringify(value)),decode=bytes=>JSON.parse(new TextDecoder().decode(bytes));

/** A bridge in memory with the same rules: the HTTP interface of `tos bridge` over the WASM journal. */
function fakeBridge({person=true}={}){
  const machine=new runtime.ActJournalV1(encode(registry)),rows=[],streams=new Set();
  const json=(value,status=200)=>new Response(JSON.stringify(value),{status,headers:{'Content-Type':'application/json'}});
  const append=drafts=>{
    const appended=[];
    for(const [index,draft] of drafts.entries()){
      try{const out=decode(machine.append(encode(draft)));rows.push({seq:out.seq,id:out.record.id,record:out.record});appended.push(out);}
      catch(error){return {appended,failure:{index,error:String(error?.message??error)}};}
    }
    for(const listener of streams)listener(rows.length);
    return {appended,failure:null};
  };
  const fetch=async(url,{method='GET',body}={})=>{
    const {pathname,searchParams}=new URL(url,'http://127.0.0.1:8742');
    const api=pathname.replace('/__bridge/v1/','');
    if(api==='hello')return json({schema:'tos.bridge.v1',acts:rows.length,paused:false,agent_tier:'propose',person});
    if(!person)return json({error:'bridge_person_only'},403);
    if(api==='acts'&&method==='GET'){
      if(searchParams.has('id'))return json({row:rows.find(row=>row.id===searchParams.get('id'))??null});
      const limit=Number(searchParams.get('limit')??100);
      if(searchParams.has('before'))return json({last:rows.length,rows:rows.filter(row=>row.seq<Number(searchParams.get('before'))).slice(-limit).reverse()});
      return json({last:rows.length,rows:rows.filter(row=>row.seq>=Number(searchParams.get('from'))).slice(0,limit)});
    }
    if(api==='acts')return json(append(JSON.parse(body)));
    if(api==='records'){const ids=JSON.parse(body).ids;return json({rows:rows.filter(row=>ids.includes(row.id))});}
    if(api==='query'){try{return json(decode(machine.query(encode(JSON.parse(body)))));}catch(error){return json({error:String(error.message)},400);}}
    if(api==='control')return json({paused:JSON.parse(body).paused??false});
    return json({error:'bridge_unknown'},404);
  };
  class EventSource{
    constructor(){this.listeners=[];this.feed=last=>{for(const fn of this.listeners)fn({data:JSON.stringify({last})});};streams.add(this.feed);}
    addEventListener(_,fn){this.listeners.push(fn);}
    close(){streams.delete(this.feed);}
  }
  return {fetch,EventSource,append,rows,machine};
}

const ticket=id=>draftAct({id,kind:'ticket',targets:['affirmation:link:01:child'],intent:'«дитяти» устарело',body:{category:'translation',variant:'outdated',observation:'слово Древа «ребёнок»'}});
const tick=()=>new Promise(resolve=>setTimeout(resolve,0));

test('the page finds its bridge only on a loopback page that the bridge recognises',async()=>{
  const bridge=fakeBridge();
  expect(await detectBridge({fetch:bridge.fetch,location:{hostname:'treeofsophia.com'}})).toBeNull();
  expect((await detectBridge({fetch:bridge.fetch,location:{hostname:'127.0.0.1'}})).schema).toBe('tos.bridge.v1');
  expect(await detectBridge({fetch:fakeBridge({person:false}).fetch,location:{hostname:'127.0.0.1'}})).toBeNull();
  expect(await detectBridge({fetch:async()=>new Response('<!doctype html>',{status:404}),location:{hostname:'localhost'}})).toBeNull();
});

test('the bridge journal appends there, reads folds and records in windows and hears agents',async()=>{
  const bridge=fakeBridge();
  const journal=createBridgeJournal({store:createBridgeStore({fetch:bridge.fetch,EventSource:bridge.EventSource})});
  expect((await journal.ready).tier).toBe('bridge');
  const events=[];journal.subscribe(event=>events.push(event.type));
  const stored=await journal.append(ticket('t1'));
  expect(stored.seq).toBe(1);
  expect((await journal.query({view:'board'})).counts.open).toBe(1);
  expect((await journal.record('t1')).body.observation).toContain('ребёнок');
  expect((await journal.records(['nope','t1'])).map(record=>record?.id??null)).toEqual([null,'t1']);
  await expect(journal.append(ticket('t1'))).rejects.toMatchObject({code:expect.stringMatching(/^act_/),index:0});
  // An agent appends over MCP: the page hears it through the event stream and reads it by its fold.
  const draft=decode(bridge.machine.agentDraft(encode({kind:'note',author:{type:'agent',id:'a1',model:'m'},targets:['III.02'],intent:'reading',body:{text:'seen'}}),'draft',Date.now(),'note-1'));
  bridge.append([draft]);await tick();
  expect(journal.size).toBe(2);
  expect(events).toContain('remote');
  expect((await journal.recent({limit:1}))[0].record.author.id).toBe('a1');
  journal.dispose();
});

test('the browser and the bridge admit an agent act alike',async()=>{
  const bridge=fakeBridge();
  const journal=createBridgeJournal({store:createBridgeStore({fetch:bridge.fetch,EventSource:bridge.EventSource})});
  await journal.append(draftAct({id:'own',kind:'note',author:{type:'agent',id:'a1',model:'m'},targets:['x'],intent:'a',body:{text:'t'}}));
  const cases=[
    [{kind:'note',author:{type:'agent',id:'a1'},targets:['x','x','y'],intent:'  two \n lines ',body:{text:'t'}},'draft'],
    [{kind:'note',author:{type:'agent',id:'a1'},targets:['x'],intent:'a',body:{text:'t'}},'read'],
    [{kind:'note',author:{type:'human',id:'a1'},targets:['x'],intent:'a',body:{text:'t'}},'propose'],
    [{kind:'review',author:{type:'agent',id:'a1'},targets:['act:own'],intent:'a',body:{decision:'accept',reason:'r'}},'propose'],
    [{kind:'ticket',author:{type:'agent',id:'a1'},targets:['x'],intent:'a',body:{category:'translation',observation:'o'}},'draft'],
    [{kind:'withdrawal',author:{type:'agent',id:'a2'},targets:['act:own'],intent:'a'},'propose'],
    [{kind:'withdrawal',author:{type:'agent',id:'a1'},targets:['act:own'],intent:'a'},'propose'],
    [{kind:'note',author:{type:'mixed',id:'a1'},targets:['x'],intent:'я'.repeat(400),body:{text:'t'}},'draft'],
  ];
  for(const [input,tier] of cases){
    const page=await agentDraft(journal,{...input,id:'same'},{tier}).then(value=>({value}),error=>({error:error.code}));
    const rust=(()=>{try{return {value:decode(bridge.machine.agentDraft(encode({...input,id:'same'}),tier,Date.parse(page.value?.at??'2026-11-20T10:00:00.000Z'),'unused'))};}catch(error){return {error:String(error?.message??error)};}})();
    expect(rust).toEqual(page);
  }
  journal.dispose();
});

test('the page keeps its own journal when no bridge serves it',async()=>{
  const made=[];
  const journal=openActJournal({detect:async()=>null,browser:()=>{const fake={ready:Promise.resolve({loaded:0}),status:{loaded:0},query:async()=>({view:'board'}),subscribe:fn=>{made.push(fn);return ()=>{};},dispose(){}};return fake;}});
  const stop=journal.subscribe(()=>{});
  expect(await journal.query({view:'board'})).toEqual({view:'board'});
  expect(made).toHaveLength(1);
  expect(journal.tier).toBe('browser');
  stop();journal.dispose();
});
