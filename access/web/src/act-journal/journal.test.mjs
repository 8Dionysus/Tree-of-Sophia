import '../observatory/human-forms-wasm-test-runtime.mjs';
import {test,expect} from 'vitest';
import {createActJournal} from './journal.mjs';
import {createMemoryActStore} from './store.mjs';
import {draftAct,actAddress} from './drafts.mjs';
import {mirrorWorkspace} from './workspace-mirror.mjs';
import {createBrowserResearchWorkspace} from '../research-workspace-rust.ts';
import {simulateArmy} from '../../fixtures/army-simulator.mjs';

const open=(store=createMemoryActStore())=>createActJournal({store,locks:null,channel:null});
const ticket=(fields={})=>draftAct({kind:'ticket',targets:['affirmation:link:01:child'],intent:'«дитяти» устарело',
  body:{category:'translation',variant:'outdated',observation:'перевод 1913 года; слово Древа «ребёнок»'},...fields});

test('a ticket is checked, stored and folded onto the board',async()=>{
  const store=createMemoryActStore(),journal=open(store);
  const stored=await journal.append(ticket({id:'t1'}));
  expect(stored.seq).toBe(1);
  expect(stored.record.digest).toMatch(/^sha256:[0-9a-f]{64}$/);
  const board=await journal.query({view:'board'});
  expect(board.counts.open).toBe(1);
  expect(board.tickets[0]).toMatchObject({id:'t1',category:'translation',status:'open'});
  expect((await journal.record('t1')).body.observation).toContain('ребёнок');
  await expect(journal.append(ticket({id:'t2',targets:[]}))).rejects.toMatchObject({code:'act_invalid:targets'});
});

test('another journal over the same store restores every record and its folds',async()=>{
  const store=createMemoryActStore(),first=open(store);
  await first.append(ticket({id:'t1'}));
  await first.append(draftAct({kind:'assignment',targets:[actAddress('t1')],intent:'to the translators',body:{assignee:'squad:translators'}}));
  const second=open(store);await second.ready;
  expect(second.size).toBe(2);
  expect((await second.query({view:'board'})).tickets[0].status).toBe('with-agent');
});

test('a changed stored record stops the journal instead of folding it',async()=>{
  const store=createMemoryActStore(),first=open(store);
  await first.append(ticket({id:'t1'}));
  const rows=await store.readFrom(1);
  const damaged=createMemoryActStore([{...rows[0],record:{...rows[0].record,intent:'changed after the fact'}}]);
  const journal=open(damaged);const status=await journal.ready;
  expect(status.damaged).toMatchObject({seq:1,error:'act_digest_mismatch'});
  await expect(journal.append(ticket({id:'t2'}))).rejects.toMatchObject({code:'act_journal_damaged'});
});

test('a refused write leaves the journal whole: the next act takes the next stored sequence, and it reopens',async()=>{
  const memory=createMemoryActStore();let refuse=1;
  const store={...memory,put:async rows=>{if(refuse-->0)throw new Error('quota exceeded');return memory.put(rows);}};
  const journal=open(store);await journal.ready;
  await expect(journal.append(ticket({id:'t1'}))).rejects.toThrow('quota exceeded');
  // The refused act is not folded: the machine holds only what the store holds.
  expect(journal.size).toBe(0);
  expect((await journal.query({view:'board'})).tickets).toHaveLength(0);
  expect((await journal.append(ticket({id:'t1'}))).seq).toBe(1);
  expect((await journal.append(ticket({id:'t2'}))).seq).toBe(2);
  const again=open(memory);const status=await again.ready;
  expect(status.damaged).toBeNull();
  expect(again.size).toBe(2);
  expect((await again.append(ticket({id:'t3'}))).seq).toBe(3);
});

test('a refused write after a refused draft keeps the earlier acts out of the machine too',async()=>{
  const memory=createMemoryActStore();let refuse=1;
  const store={...memory,put:async rows=>{if(refuse-->0)throw new Error('disk full');return memory.put(rows);}};
  const journal=open(store);await journal.ready;
  await expect(journal.appendMany([ticket({id:'t1'}),ticket({id:'t2',targets:[]})])).rejects.toThrow('disk full');
  expect(journal.size).toBe(0);
  expect((await journal.append(ticket({id:'t1'}))).seq).toBe(1);
  const again=open(memory);expect((await again.ready).damaged).toBeNull();
  expect(again.size).toBe(1);
});

test('the workspace journal is mirrored once',async()=>{
  const workspace=createBrowserResearchWorkspace({persistence:false});
  const journal=open();await journal.ready;
  const stop=mirrorWorkspace(workspace,journal,{sessionId:'test'});
  workspace.addHypothesis({id:'h1',title:'The child is the yes',body:'A reading',targetId:'child'});
  await stop();
  const summary=await journal.query({view:'summary'});
  expect(summary.by_kind.find(row=>row.kind==='workspace-event')?.count).toBeGreaterThan(0);
  const before=journal.size;
  const again=mirrorWorkspace(workspace,journal,{sessionId:'test'});await again();
  expect(journal.size).toBe(before);
});

test('an army of fifty agents folds into board, review, leases and presence',async()=>{
  const {acts,proposals}=simulateArmy({agents:50,acts:2000,seed:7});
  expect(simulateArmy({agents:50,acts:2000,seed:7}).acts[100]).toEqual(acts[100]);
  const journal=open();await journal.ready;
  for(let at=0;at<acts.length;at+=500)await journal.appendMany(acts.slice(at,at+500));
  expect(journal.size).toBe(2000);
  const summary=await journal.query({view:'summary'});
  expect(summary.agents).toBe(50);
  expect(summary.models).toBe(4);
  const review=await journal.query({view:'review',limit:10});
  expect(review.open_total).toBeGreaterThan(0);
  expect(review.open_total).toBeLessThanOrEqual(proposals.length);
  // Repeated changes gather: there are fewer groups than open proposals.
  expect(review.groups_total).toBeLessThan(review.open_total);
  const last=new Date(acts[acts.length-1].at).getTime();
  const presence=await journal.query({view:'presence',now_ms:last,window_ms:60000});
  expect(presence.agents).toBeGreaterThan(0);
  const leases=await journal.query({view:'leases',now_ms:last});
  expect(leases.leases.length).toBeGreaterThan(0);
});
