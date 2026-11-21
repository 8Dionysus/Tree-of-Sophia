import '../observatory/human-forms-wasm-test-runtime.mjs';
import {test,expect} from 'vitest';
import {createActJournal} from './journal.mjs';
import {createMemoryActStore} from './store.mjs';
import {actJournalTools,actManifest} from './agent-tools.mjs';

const agent={type:'agent',id:'reader-1',model:'model-a',role:'reader'};
const setup=(tier='propose')=>{
  const journal=createActJournal({store:createMemoryActStore(),locks:null,channel:null});
  const tools=Object.fromEntries(actJournalTools(journal,{tier:()=>tier}).map(tool=>[tool.name,tool]));
  const call=async(name,input)=>JSON.parse((await tools[name].execute(input,{signal:new AbortController().signal})).content[0].text);
  return {journal,tools,call};
};

test('the manifest names every kind with who may append it, the commands and the tier',async()=>{
  const {journal}=setup();
  const manifest=await actManifest(journal,{tier:'draft'});
  expect(manifest.tier).toBe('draft');
  expect(manifest.kinds.find(kind=>kind.name==='review').agent).toBe('person-only');
  expect(manifest.kinds.find(kind=>kind.name==='proposal').agent).toBe('propose-tier');
  expect(manifest.kinds.find(kind=>kind.name==='lease').agent).toBe('draft-tier');
  expect(manifest.addressed_commands.map(command=>command.command_id)).toContain('tos.act.append');
  expect(manifest.pointing_commands.map(command=>command.command_id)).toContain('tos.page.select');
});

test('an agent appends in its own name only, and never a decision',async()=>{
  const {call}=setup();
  const lease=await call('tos.act.append',{kind:'lease',author:agent,targets:['III.02'],intent:'reading the gate',body:{ttl_ms:60000}});
  expect(lease.seq).toBe(1);
  await expect(call('tos.act.append',{kind:'note',author:{type:'human',id:'someone'},targets:['x'],intent:'x',body:{text:'x'}})).rejects.toMatchObject({code:'act_author'});
  await expect(call('tos.act.append',{kind:'review',author:agent,targets:['act:x'],intent:'x',body:{decision:'accept',reason:'x'}})).rejects.toMatchObject({code:'act_person_only'});
  // A second agent on the same place learns of the first one's lease.
  const second=await call('tos.act.append',{kind:'lease',author:{...agent,id:'reader-2'},targets:['III.02'],intent:'also reading',body:{ttl_ms:60000}});
  expect(second.collisions).toEqual(['III.02']);
});

test('the draft tier cannot propose, and an agent withdraws only its own acts',async()=>{
  const {call}=setup('draft');
  await expect(call('tos.act.append',{kind:'proposal',author:agent,targets:['x'],grounds:['g'],intent:'x',body:{category:'open',change:{a:1}}})).rejects.toMatchObject({code:'act_tier'});
  const note=await call('tos.act.append',{kind:'note',author:agent,targets:['x'],intent:'a note',body:{text:'x'}});
  await expect(call('tos.act.append',{kind:'withdrawal',author:{...agent,id:'other'},targets:[`act:${note.id}`],intent:'undo',body:{}})).rejects.toMatchObject({code:'act_not_own'});
  const undone=await call('tos.act.append',{kind:'withdrawal',author:agent,targets:[`act:${note.id}`],intent:'undo',body:{}});
  expect(undone.seq).toBe(2);
});

test('a batch reports its first refusal with what was appended before it',async()=>{
  const {call}=setup();
  const result=await call('tos.act.append-batch',{acts:[
    {kind:'note',author:agent,targets:['a'],intent:'one',body:{text:'1'}},
    {kind:'note',author:agent,targets:[],intent:'two',body:{text:'2'}},
  ]});
  expect(result.appended).toHaveLength(1);
  expect(result.failure).toMatchObject({index:1,error:'act_invalid:targets'});
  const summary=await call('tos.act.query',{view:'summary'});
  expect(summary.acts).toBe(1);
});
